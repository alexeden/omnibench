#[cfg(feature = "relay")]
use embedded_hal_bus::i2c::RefCellDevice;
#[cfg(feature = "relay")]
use esp_idf_svc::hal::{
    gpio::PinDriver,
    i2c::{I2cConfig, I2cDriver},
    units::FromValueType,
};
use esp_idf_svc::{
    bt::{
        BtDriver,
        ble::{gap::EspBleGap, gatt::server::EspGatts},
    },
    hal::peripherals::Peripherals,
    nvs::EspDefaultNvsPartition,
};
use log::*;
use omnibench::{
    APP_ID,
    protocol::{ClientEvent, RelayState},
    server::OmnibenchServer,
    stepper::Stepper,
};
#[cfg(feature = "relay")]
use port_expander::{Pcf8574a, write_multiple};
#[cfg(feature = "relay")]
use std::cell::RefCell;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn main() -> anyhow::Result<()> {
    esp_idf_svc::hal::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let bt = Arc::new(BtDriver::new(peripherals.modem, Some(nvs.clone()))?);

    // I2C
    #[cfg(feature = "relay")]
    let i2c = {
        let peripherals = Peripherals::take()?;
        let (i2c_power, sda, scl) = omnibench::board_i2c_pins!(peripherals);
        let mut i2c_power = PinDriver::output(i2c_power)?;
        i2c_power.set_low()?;
        let config = I2cConfig::new().baudrate(400u32.kHz().into());
        let i2c = RefCell::new(I2cDriver::<'static>::new(
            peripherals.i2c0,
            sda,
            scl,
            &config,
        )?);
        i2c_power.set_high()?;
        std::thread::sleep(Duration::from_millis(50));
        i2c
    };
    #[cfg(feature = "relay")]
    let mut relays = Pcf8574a::new(RefCellDevice::new(&i2c), true, true, true);

    let (stepper_dir, stepper_en, stepper_pul) = omnibench::board_stepper_pins!(peripherals);

    // BLE
    let server = OmnibenchServer::new(
        Arc::new(EspBleGap::new(bt.clone())?),
        Arc::new(EspGatts::new(bt.clone())?),
    );
    let gap_server = server.clone();
    let gatts_server = server.clone();
    server
        .gap
        .subscribe(move |event| {
            gap_server.check_esp_status(gap_server.on_gap_event(event));
        })
        .expect("Failed to subscribe to Gap events");
    server
        .gatts
        .subscribe(move |(gatt_if, event)| {
            gatts_server.check_esp_status(gatts_server.on_gatts_event(gatt_if, event))
        })
        .expect("Failed to subscribe to Gatts events");
    info!("BLE Gap and Gatts subscriptions initialized");
    server
        .gatts
        .register_app(APP_ID)
        .expect("Failed to register Gatts app");
    info!("Gatts BTP app registered");

    let relay_state = Arc::new(Mutex::new(RelayState::default()));
    let joy_state = Arc::new(Mutex::new(0i8));

    // Stepper thread: construct and drive entirely within the thread to avoid
    // Send issues with rmt_encoder_t.
    let joy_state_stepper = joy_state.clone();
    std::thread::spawn(move || {
        let mut stepper = match Stepper::try_new(stepper_dir, stepper_en, stepper_pul) {
            Ok(s) => s,
            Err(e) => {
                error!("Stepper init failed: {e:?}");
                panic!("Stepper init failed: {e:?}");
            }
        };
        info!("Stepper initialized");
        let mut last_joy = 0i8;
        loop {
            let joy = *joy_state_stepper.lock().unwrap();
            if joy != last_joy {
                if let Err(e) = stepper.drive(joy) {
                    warn!("Stepper error: {e:?}");
                }
                last_joy = joy;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    // Toggle the appropriate relay when a ButtonEvent arrives from the client,
    // then broadcast the updated state to all subscribers.
    let relay_state_recv = relay_state.clone();
    let joy_state_recv = joy_state.clone();
    let server_recv = server.clone();
    server.set_recv_callback(move |bytes| match ClientEvent::from_bytes(bytes) {
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
            *joy_state_recv.lock().unwrap() = event.value;
            // info!("Joystick: {}", event.value);
        }
        None => {
            warn!("Unknown recv payload: {bytes:?}");
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
    info!("Starting main loop...");

    loop {
        let current_state = *relay_state.lock().unwrap();
        let connected = server.has_connections();

        if Some(current_state) != last_relay_state || Some(connected) != last_connected {
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

        std::thread::sleep(Duration::from_millis(19));
    }
}
