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
};
use std::{sync::Arc, time::Duration};

const RED: NeoKey1x4Color = NeoKey1x4Color { r: 255, g: 0, b: 0 };
const BLUE: NeoKey1x4Color = NeoKey1x4Color { r: 0, g: 0, b: 255 };
const WHITE: NeoKey1x4Color = NeoKey1x4Color {
    r: 255,
    g: 255,
    b: 255,
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

    info!("Subscriptions initialized; registering app and starting scan");
    client.gattc.register_app(APP_ID)?;
    gatt::set_local_mtu(500)?;

    // Main loop: update LEDs from connection status, handle button presses for
    // rescan.
    let mut last_status = None;
    // Tracks whether a button was already pressed this gesture, to avoid
    // re-triggering.
    let mut rescan_pending = false;

    loop {
        let status = client.status();

        // Update LEDs only when status changes.
        if Some(status) != last_status {
            let color = match status {
                ConnectionStatus::Scanning => BLUE,
                ConnectionStatus::Connected => WHITE,
                ConnectionStatus::ScanFailed | ConnectionStatus::Disconnected => RED,
                ConnectionStatus::Error => ORANGE,
            };
            info!("Connection status: {status:?}");
            neokeys
                .set_neopixel_colors(&[color, color, color, color])
                .and_then(|_| neokeys.sync_neopixel())
                .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))?;
            last_status = Some(status);
        }

        // When not connected, any button press restarts the scan.
        if matches!(
            status,
            ConnectionStatus::ScanFailed | ConnectionStatus::Disconnected | ConnectionStatus::Error
        ) {
            // keys() returns a bitmask; 0 = pressed (active low), 4 bits for 4 keys.
            if let Ok(keys) = neokeys.keys() {
                let any_pressed = keys & 0xF != 0xF;
                if any_pressed && !rescan_pending {
                    rescan_pending = true;
                    info!("Button pressed — restarting scan");
                    client.connect()?;
                } else if !any_pressed {
                    rescan_pending = false;
                }
            }
        } else {
            rescan_pending = false;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
