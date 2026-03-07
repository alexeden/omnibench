use esp_idf_svc::hal::{gpio::PinDriver, peripherals::Peripherals};
use std::time::Duration;

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let mut pins_left = [
        PinDriver::output(peripherals.pins.gpio13)?, // A12
        PinDriver::output(peripherals.pins.gpio12)?, // A11
        PinDriver::output(peripherals.pins.gpio27)?, // A10
        PinDriver::output(peripherals.pins.gpio33)?, // A9
        PinDriver::output(peripherals.pins.gpio15)?, // A8
        PinDriver::output(peripherals.pins.gpio32)?, // A7
        PinDriver::output(peripherals.pins.gpio14)?, // A6
    ];
    let mut pins_right = [
        PinDriver::output(peripherals.pins.gpio26)?, // A0
        PinDriver::output(peripherals.pins.gpio25)?, // A1
        // PinDriver::output(peripherals.pins.gpio34)?, // A2
        // PinDriver::output(peripherals.pins.gpio39)?, // A3
        // PinDriver::output(peripherals.pins.gpio36)?, // A4
        PinDriver::output(peripherals.pins.gpio4)?,  // A5
        PinDriver::output(peripherals.pins.gpio20)?, // SCL
        PinDriver::output(peripherals.pins.gpio19)?, // MOSI
        PinDriver::output(peripherals.pins.gpio21)?, // MISO
        PinDriver::output(peripherals.pins.gpio7)?,  // RX
        PinDriver::output(peripherals.pins.gpio8)?,  // TX
    ];
    // let (dir, en, pul) = omnibench::board_stepper_pins!(peripherals);
    // let mut dir = PinDriver::output(dir)?;
    // let mut en = PinDriver::output(en)?;
    // let mut pul = PinDriver::output(pul)?;

    let labels = [
        "A12/IO13", "A11/IO12", "A10/IO27", "A9/IO33", "A8/IO15", "A7/IO32", "A6/IO14",
    ];
    loop {
        // for i in 0..pins.len() {
        //     log::info!("HIGH: {}", labels[i]);
        for pin in pins_left.iter_mut().chain(pins_right.iter_mut()) {
            pin.set_high()?;
        }
        // } else {
        // pin.set_low()?;
        // }
        std::thread::sleep(Duration::from_millis(1000));
        for pin in pins_left.iter_mut().chain(pins_right.iter_mut()) {
            pin.set_low()?;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
