use esp_idf_svc::hal::{
    gpio::PinDriver,
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    units::FromValueType,
};
use log::info;
use omnibench::joystick::QwiicJoy;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    let mut i2c_power = PinDriver::output(peripherals.pins.gpio7)?;
    i2c_power.set_low()?;
    std::thread::sleep(Duration::from_millis(50));

    let (sda, scl) = (peripherals.pins.gpio3, peripherals.pins.gpio4);
    let config = I2cConfig::new().baudrate(400u32.kHz().into());
    let i2c = I2cDriver::<'static>::new(peripherals.i2c0, sda, scl, &config)?;

    i2c_power.set_high()?;
    std::thread::sleep(Duration::from_millis(100));

    let mut joy = QwiicJoy::new(i2c);

    info!("Joystick initialized");
    info!(
        "Default address: 0x{:02X}",
        joy.default_address()
            .expect("Failed to read default address")
    );

    loop {
        match joy.state() {
            Ok(state) => info!("x={:4}  y={:4}  btn={}", state.x, state.y, state.btn),
            Err(e) => info!("Error reading state: {:?}", e),
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
