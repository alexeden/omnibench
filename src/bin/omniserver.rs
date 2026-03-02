use adafruit_seesaw::{
    SeesawDriver,
    devices::{NeoTrellis, NeoTrellisColor},
    prelude::*,
};
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
    protocol::{ButtonEvent, RelayState},
    server::OmnibenchServer,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const WHITE: NeoTrellisColor = NeoTrellisColor {
    r: 255,
    g: 255,
    b: 255,
};
const DIM_WHITE: NeoTrellisColor = NeoTrellisColor {
    r: 15,
    g: 15,
    b: 15,
};

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
    let i2c = I2cDriver::<'static>::new(peripherals.i2c0, sda, scl, &config)?;
    i2c_power.set_high()?;
    std::thread::sleep(Duration::from_millis(50));
    let seesaw = SeesawDriver::new(delay, i2c);

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
        info!("Server got gap event: {event:?}");
        gap_server.check_esp_status(gap_server.on_gap_event(event));
    })?;

    let gatts_server = server.clone();

    server.gatts.subscribe(move |(gatt_if, event)| {
        info!("Server got gatts event: {event:?}");
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
        if let Some(event) = ButtonEvent::from_bytes(bytes) {
            let new_state = {
                let mut rs = relay_state_recv.lock().unwrap();
                *rs = rs.toggle(event.relay);
                *rs
            };
            info!("Relay {} toggled → state {:?}", event.relay, new_state);
            server_recv.check_esp_status(server_recv.indicate(&new_state.to_bytes()));
        }
    });

    // Send the current relay state immediately when a client subscribes.
    let relay_state_sub = relay_state.clone();
    let server_sub = server.clone();
    server.set_subscribed_callback(move || {
        let state = *relay_state_sub.lock().unwrap();
        info!("Client subscribed — sending current relay state");
        server_sub.check_esp_status(server_sub.indicate(&state.to_bytes()));
    });

    // Main loop: update NeoTrellis LEDs whenever the relay state changes.
    // Each relay maps to one row of 4 LEDs (relay 0 = row 0, …, relay 3 = row 3).
    let mut last_relay_state: Option<RelayState> = None;

    loop {
        let current_state = *relay_state.lock().unwrap();

        if Some(current_state) != last_relay_state {
            let colors: [NeoTrellisColor; 16] = std::array::from_fn(|i| {
                if current_state.is_on((i / 4) as u8) {
                    WHITE
                } else {
                    DIM_WHITE
                }
            });
            trellis
                .set_neopixel_colors(&colors)
                .and_then(|_| trellis.sync_neopixel())
                .expect("Failed to update NeoTrellis LEDs");
            last_relay_state = Some(current_state);
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
