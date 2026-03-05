use esp_idf_svc::hal::{peripherals::Peripherals, units::FromValueType};
use log::*;
use omnibench::stepper::FreqGen;
use std::time::Duration;

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    let mut freq_gen = FreqGen::try_new(peripherals.pins.gpio13)?;
    info!("FreqGen initialized");

    let mut f = 1u32.Hz();
    loop {
        if let Err(e) = freq_gen.set_freq(f) {
            error!("Error setting frequency: {:?}", e);
        }
        f = if f > 20.Hz() {
            1u32.Hz()
        } else {
            f + 1u32.Hz()
        };
        std::thread::sleep(Duration::from_millis(500));
    }
}
