use adafruit_seesaw::{SeesawDriver, devices::NeoTrellis, prelude::*};
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
use omnibench::{APP_ID, server::OmnibenchServer};
use std::{sync::Arc, time::Duration};

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

    let mut ind_data = 0_u16;

    loop {
        server.indicate(&ind_data.to_le_bytes())?;
        info!("Broadcasted indication: {ind_data}");

        ind_data = ind_data.wrapping_add(1);

        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
