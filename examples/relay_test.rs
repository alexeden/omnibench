use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::hal::{
    gpio::PinDriver,
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    units::FromValueType,
};
use log::*;
use port_expander::{Pcf8574a, write_multiple};
use std::{cell::RefCell, time::Duration};

fn main() -> anyhow::Result<()> {
    esp_idf_svc::hal::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    // I2C
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
    let mut relays = Pcf8574a::new(RefCellDevice::new(&i2c), true, true, true);

    info!("Starting main loop...");

    let mut even = true;
    loop {
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
            std::array::from_fn(|i| if even { i % 2 == 0 } else { i % 2 == 1 }),
        )?;

        even = !even;
        std::thread::sleep(Duration::from_millis(1_000));
    }
}
