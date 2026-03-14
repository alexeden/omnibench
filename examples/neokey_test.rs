use adafruit_seesaw::{SeesawDriver, devices::NeoKey1x4, prelude::*};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::hal::{
    delay::Delay,
    gpio::PinDriver,
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    units::FromValueType,
};
use omnibench::colors::{BLUE, OFF, RED, WHITE};
use std::{cell::RefCell, time::Duration};

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    let (i2c_power, sda, scl) = omnibench::board_i2c_pins!(peripherals);
    let mut i2c_power = PinDriver::output(i2c_power)?;
    i2c_power.set_low()?;
    let config = I2cConfig::new().baudrate(400u32.kHz().into());
    let delay = Delay::new_default();
    let i2c = RefCell::new(I2cDriver::<'static>::new(
        peripherals.i2c1,
        sda,
        scl,
        &config,
    )?);
    i2c_power.set_high()?;
    std::thread::sleep(Duration::from_millis(50));

    let seesaw = SeesawDriver::new(delay, RefCellDevice::new(&i2c));
    let mut neokeys1 = NeoKey1x4::new(0x33, seesaw)
        .init()
        .expect("Failed to init NeoKey1x4 @ 0x33");
    neokeys1
        .enable_interrupt()
        .expect("Failed to enable interrupts");
    let seesaw = SeesawDriver::new(delay, RefCellDevice::new(&i2c));
    let mut neokeys2 = NeoKey1x4::new_with_default_addr(seesaw)
        .init()
        .expect("Failed to init NeoKey1x4 @ default addr");
    neokeys2
        .enable_interrupt()
        .expect("Failed to enable interrupts");
    log::info!("NeoKeys initialized — cycling colors and printing key presses");

    let colors = [RED, WHITE, BLUE];
    let mut color_idx = 0usize;
    let mut last_keys: u8 = 0;

    loop {
        // Read keys from both boards
        let k0 = neokeys1.keys().unwrap_or(last_keys & 0x0F) & 0x0F;
        let k1 = neokeys2.keys().unwrap_or(last_keys >> 4) & 0x0F;
        let keys = k0 | (k1 << 4);

        if keys != last_keys {
            log::info!("Keys: board1={:#06b} board2={:#06b}", k0, k1);
            last_keys = keys;

            // Advance color on any press
            if keys != 0 {
                color_idx = (color_idx + 1) % colors.len();
            }
        }

        // Light pressed keys with the current color, unpressed off
        let strip1: [_; 4] = std::array::from_fn(|i| {
            if k0 & (1 << i) != 0 {
                colors[color_idx]
            } else {
                OFF
            }
        });
        let strip2: [_; 4] = std::array::from_fn(|i| {
            if k1 & (1 << i) != 0 {
                colors[color_idx]
            } else {
                OFF
            }
        });

        neokeys1
            .set_neopixel_colors(&strip1)
            .and_then(|_| neokeys1.sync_neopixel())
            .ok();
        neokeys2
            .set_neopixel_colors(&strip2)
            .and_then(|_| neokeys2.sync_neopixel())
            .ok();

        std::thread::sleep(Duration::from_millis(20));
    }
}
