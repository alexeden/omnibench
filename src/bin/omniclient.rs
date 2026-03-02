use adafruit_seesaw::{
    SeesawDriver,
    devices::{NeoKey1x4, NeoKey1x4Color},
    prelude::*,
};
use esp_idf_svc::{
    bt::{
        BtDriver,
        ble::{
            gap::EspBleGap,
            gatt::{self, client::EspGattc},
        },
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
    client::{ConnectionStatus, OmnibenchClient},
    protocol::{ButtonEvent, RelayState},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const RED: NeoKey1x4Color = NeoKey1x4Color { r: 255, g: 0, b: 0 };
const BLUE: NeoKey1x4Color = NeoKey1x4Color { r: 0, g: 0, b: 255 };
const WHITE: NeoKey1x4Color = NeoKey1x4Color {
    r: 255,
    g: 255,
    b: 255,
};
const DIM_WHITE: NeoKey1x4Color = NeoKey1x4Color {
    r: 15,
    g: 15,
    b: 15,
};
const ORANGE: NeoKey1x4Color = NeoKey1x4Color {
    r: 255,
    g: 128,
    b: 0,
};

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

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
    let mut neokeys = NeoKey1x4::new(0x33, seesaw)
        .init()
        .expect("Failed to start NeoKey1x4");

    info!("Seesaw initialized");

    // Start blue — scanning is about to begin
    neokeys
        .set_neopixel_colors(&[BLUE, BLUE, BLUE, BLUE])
        .and_then(|_| neokeys.sync_neopixel())
        .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))?;

    let nvs = EspDefaultNvsPartition::take()?;
    let bt = Arc::new(BtDriver::new(peripherals.modem, Some(nvs.clone()))?);
    let client = OmnibenchClient::new(
        Arc::new(EspBleGap::new(bt.clone())?),
        Arc::new(EspGattc::new(bt.clone())?),
    );
    info!("BLE Gap and Gattc initialized");

    let relay_state = Arc::new(Mutex::new(RelayState::default()));

    let gap_client = client.clone();
    client.gap.subscribe(move |event| {
        if let Err(e) = gap_client.on_gap_event(event) {
            warn!("Gap event error: {e:?}");
        }
    })?;

    let gattc_client = client.clone();
    client.gattc.subscribe(move |(gatt_if, event)| {
        if let Err(e) = gattc_client.on_gattc_event(gatt_if, event) {
            warn!("Gattc event error: {e:?}");
        }
    })?;

    let relay_state_cb = relay_state.clone();
    client.set_notify_callback(move |bytes| {
        if let Some(rs) = RelayState::from_bytes(bytes) {
            *relay_state_cb.lock().unwrap() = rs;
        }
    });

    info!("Subscriptions initialized; registering app and starting scan");
    client.gattc.register_app(APP_ID)?;
    gatt::set_local_mtu(500)?;

    // Main loop: update LEDs from connection status and relay state; handle
    // button presses for both relay toggling (connected) and rescan (disconnected).
    let mut last_status = None;
    let mut last_relay_state: Option<RelayState> = None;
    // Previous key bitmask for edge detection (0 = pressed, active low).
    // Start all-high (no keys pressed).
    let mut last_keys: u8 = 0xF;

    loop {
        let status = client.status();
        let current_relay_state = *relay_state.lock().unwrap();
        // Fall back to last known state on I2C error.
        let keys = neokeys.keys().unwrap_or(last_keys);

        // Update LEDs when connection status, relay state, or key state changes.
        let leds_stale = Some(status) != last_status
            || (status == ConnectionStatus::Connected
                && (Some(current_relay_state) != last_relay_state || keys != last_keys));

        if leds_stale {
            info!("Connection status: {status:?}  Relay state: {current_relay_state:?}");
            match status {
                ConnectionStatus::Connected => {
                    // Pressed buttons (bit = 0) show blue; others reflect relay state.
                    let colors = [
                        if (keys & 1) == 0 {
                            BLUE
                        } else if current_relay_state.is_on(0) {
                            WHITE
                        } else {
                            DIM_WHITE
                        },
                        if (keys >> 1) & 1 == 0 {
                            BLUE
                        } else if current_relay_state.is_on(1) {
                            WHITE
                        } else {
                            DIM_WHITE
                        },
                        if (keys >> 2) & 1 == 0 {
                            BLUE
                        } else if current_relay_state.is_on(2) {
                            WHITE
                        } else {
                            DIM_WHITE
                        },
                        if (keys >> 3) & 1 == 0 {
                            BLUE
                        } else if current_relay_state.is_on(3) {
                            WHITE
                        } else {
                            DIM_WHITE
                        },
                    ];
                    neokeys
                        .set_neopixel_colors(&colors)
                        .and_then(|_| neokeys.sync_neopixel())
                        .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))?;
                }
                _ => {
                    let color = match status {
                        ConnectionStatus::Scanning => BLUE,
                        ConnectionStatus::ScanFailed | ConnectionStatus::Disconnected => RED,
                        ConnectionStatus::Error => ORANGE,
                        ConnectionStatus::Connected => unreachable!(),
                    };
                    neokeys
                        .set_neopixel_colors(&[color, color, color, color])
                        .and_then(|_| neokeys.sync_neopixel())
                        .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))?;
                }
            }
            last_status = Some(status);
            last_relay_state = Some(current_relay_state);
        }

        // Button handling — detect falling edge (bit 1 → 0 = key just pressed).
        match status {
            ConnectionStatus::Connected => {
                for i in 0..4u8 {
                    let bit = 1u8 << i;
                    if (last_keys & bit) != 0 && (keys & bit) == 0 {
                        info!("Button {i} pressed — toggling relay {i}");
                        client.write_characteristic(&ButtonEvent { relay: i }.to_bytes())?;
                    }
                }
            }
            ConnectionStatus::ScanFailed
            | ConnectionStatus::Disconnected
            | ConnectionStatus::Error => {
                if last_keys & 0xF == 0xF && keys & 0xF != 0xF {
                    info!("Button pressed — restarting scan");
                    client.connect()?;
                }
            }
            ConnectionStatus::Scanning => {}
        }
        last_keys = keys;

        std::thread::sleep(Duration::from_millis(1));
    }
}
