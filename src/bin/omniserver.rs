use adafruit_seesaw::{
    SeesawDriver,
    devices::{NeoTrellis, NeoTrellisColor},
    prelude::*,
};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::{
    bt::{
        BtDriver,
        ble::{gap::EspBleGap, gatt::server::EspGatts},
    },
    hal::{
        delay::Delay,
        gpio::PinDriver,
        i2c::{I2cConfig, I2cDriver},
        peripherals::Peripherals,
        units::FromValueType,
    },
    nvs::EspDefaultNvsPartition,
};
use log::*;
use omnibench::{
    APP_ID,
    protocol::{ClientEvent, RelayState},
    server::OmnibenchServer,
};
#[cfg(feature = "relay")]
use port_expander::{Pcf8574a, write_multiple};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    time::Duration,
};

const WHITE: NeoTrellisColor = NeoTrellisColor {
    r: 255,
    g: 255,
    b: 255,
};
const DIM_WHITE: NeoTrellisColor = NeoTrellisColor { r: 2, g: 2, b: 2 };
const RED: NeoTrellisColor = NeoTrellisColor { r: 255, g: 0, b: 0 };
const DIM_RED: NeoTrellisColor = NeoTrellisColor { r: 5, g: 0, b: 0 };

fn main() -> anyhow::Result<()> {
    esp_idf_svc::hal::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let bt = Arc::new(BtDriver::new(peripherals.modem, Some(nvs.clone()))?);

    // I2C
    let mut i2c_power = PinDriver::output(peripherals.pins.gpio7)?;
    i2c_power.set_low()?;

    info!("Initializing I2C and Seesaw");
    let (sda, scl) = (peripherals.pins.gpio3, peripherals.pins.gpio4);
    let config = I2cConfig::new().baudrate(400u32.kHz().into());
    let delay = Delay::new_default();
    let i2c = RefCell::new(I2cDriver::<'static>::new(
        peripherals.i2c0,
        sda,
        scl,
        &config,
    )?);
    i2c_power.set_high()?;

    #[cfg(feature = "relay")]
    let mut relays = Pcf8574a::new(RefCellDevice::new(&i2c), true, true, true);

    std::thread::sleep(Duration::from_millis(50));
    let seesaw = SeesawDriver::new(delay, RefCellDevice::new(&i2c));

    let mut trellis = NeoTrellis::new_with_default_addr(seesaw)
        .init()
        .expect("Failed to start NeoTrellis");

    // Listen for key presses
    for x in 0..trellis.num_cols() {
        for y in 0..trellis.num_rows() {
            trellis
                .set_key_event_triggers(x, y, &[KeyEventType::Pressed], true)
                .expect("Failed to set key event triggers");
        }
    }

    let server = OmnibenchServer::new(
        Arc::new(EspBleGap::new(bt.clone())?),
        Arc::new(EspGatts::new(bt.clone())?),
    );

    info!("BLE Gap and Gatts initialized");

    let gap_server = server.clone();

    server.gap.subscribe(move |event| {
        // info!("Server got gap event: {event:?}");
        gap_server.check_esp_status(gap_server.on_gap_event(event));
    })?;

    let gatts_server = server.clone();

    server.gatts.subscribe(move |(gatt_if, event)| {
        // info!("Server got gatts event: {event:?}");
        gatts_server.check_esp_status(gatts_server.on_gatts_event(gatt_if, event))
    })?;

    info!("BLE Gap and Gatts subscriptions initialized");

    server.gatts.register_app(APP_ID)?;

    info!("Gatts BTP app registered");

    let relay_state = Arc::new(Mutex::new(RelayState::default()));

    // Toggle the appropriate relay when a ButtonEvent arrives from the client,
    // then broadcast the updated state to all subscribers.
    let relay_state_recv = relay_state.clone();
    let server_recv = server.clone();
    server.set_recv_callback(move |bytes| {
        match ClientEvent::from_bytes(bytes) {
            Some(ClientEvent::Button(event)) => {
                let new_state = {
                    let mut rs = relay_state_recv.lock().unwrap();
                    *rs = rs.toggle(event.relay);
                    *rs
                };
                info!("Relay {} toggled → state {:?}", event.relay, new_state);
                server_recv.check_esp_status(server_recv.notify(&new_state.to_bytes()));
            }
            Some(ClientEvent::Joystick(event)) => {
                info!("Joystick: {}", event.value);
            }
            None => {
                warn!("Unknown recv payload: {bytes:?}");
            }
        }
    });

    // Send the current relay state immediately when a client subscribes.
    let relay_state_sub = relay_state.clone();
    let server_sub = server.clone();
    server.set_subscribed_callback(move || {
        let state = *relay_state_sub.lock().unwrap();
        info!("Client subscribed — sending current relay state");
        server_sub.check_esp_status(server_sub.notify(&state.to_bytes()));
    });

    // Main loop: update NeoTrellis LEDs whenever the relay state or connection
    // state changes. Each relay maps to 2 LEDs. Color scheme: white = connected,
    // red = no clients connected. Brightness reflects relay on/off state.
    let mut last_relay_state: Option<RelayState> = None;
    let mut last_connected: Option<bool> = None;

    loop {
        let current_state = *relay_state.lock().unwrap();
        let connected = server.has_connections();

        if Some(current_state) != last_relay_state || Some(connected) != last_connected {
            let (on_color, off_color) = if connected {
                (WHITE, DIM_WHITE)
            } else {
                (RED, DIM_RED)
            };
            let colors: [NeoTrellisColor; 16] = std::array::from_fn(|i| {
                if current_state.is_on((i / 2) as u8) {
                    on_color
                } else {
                    off_color
                }
            });
            trellis
                .set_neopixel_colors(&colors)
                .and_then(|_| trellis.sync_neopixel())
                .expect("Failed to update NeoTrellis LEDs");
            #[cfg(feature = "relay")]
            {
                let mut pins = relays.split();
                write_multiple(
                    [
                        &mut pins.p0,
                        &mut pins.p1,
                        &mut pins.p2,
                        &mut pins.p3,
                        &mut pins.p4,
                        &mut pins.p5,
                        &mut pins.p6,
                        &mut pins.p7,
                    ],
                    std::array::from_fn(|i| current_state.is_on(i as u8)),
                )?;
            }

            last_relay_state = Some(current_state);
            last_connected = Some(connected);
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
