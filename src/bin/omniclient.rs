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
use omnibench::{APP_ID, client::OmnibenchClient};
use std::{sync::Arc, time::Duration};

const RED: NeoKey1x4Color = NeoKey1x4Color { r: 255, g: 0, b: 0 };
const GREEN: NeoKey1x4Color = NeoKey1x4Color { r: 0, g: 255, b: 0 };

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    // I2C
    let mut i2c_power = PinDriver::output(peripherals.pins.gpio7)?;
    i2c_power.set_low()?;

    // I2C
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

    neokeys
        .set_neopixel_colors(&[
            RED, // if keys & 1 == 0 { GREEN } else { RED },
            RED, // if (keys >> 1) & 1 == 0 { GREEN } else { RED },
            RED, // if (keys >> 2) & 1 == 0 { GREEN } else { RED },
            RED, // if (keys >> 3) & 1 == 0 { GREEN } else { RED },
        ])
        .and_then(|_| neokeys.sync_neopixel())?;

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
            warn!("Got status: {e:?}");
        }
    })?;

    let gattc_client = client.clone();

    client.gattc.subscribe(move |(gatt_if, event)| {
        info!("Got gattc event: {event:?}");
        if let Err(e) = gattc_client.on_gattc_event(gatt_if, event) {
            warn!("Got status: {e:?}");
        }
    })?;

    info!("BLE Gap and Gattc subscriptions initialized");

    client.gattc.register_app(APP_ID)?;

    info!("Gattc BTP app registered");

    gatt::set_local_mtu(500)?;

    info!("Gattc BTP app registered");

    client.wait_for_write_char_handle();
    let mut i = 0_u16;
    let mut indicate = true;

    info!("Client initialized, looping");

    loop {
        // Subscribe/unsubscribe to indications
        if i.is_multiple_of(10) {
            client.request_indicate(indicate)?;
            indicate = !indicate;
        }

        client.write_characterisitic(&i.to_le_bytes())?;

        info!("Wrote characteristic: {i}");

        i = i.wrapping_add(1);

        std::thread::sleep(Duration::from_secs(5));

        if i.is_multiple_of(30) {
            client.disconnect()?;
            std::thread::sleep(Duration::from_secs(5));
            client.connect()?;
        }
    }
}
