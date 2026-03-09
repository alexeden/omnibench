use esp_idf_svc::hal::peripherals::Peripherals;
use log::*;
use omnibench::stepper::{RampConfig, Stepper};
use std::time::Duration;

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let (stepper_dir, stepper_en, stepper_pul) = omnibench::board_stepper_pins!(peripherals);

    let mut _stepper =
        match Stepper::try_new(stepper_dir, stepper_en, stepper_pul, RampConfig::default()) {
            Ok(s) => s,
            Err(e) => {
                error!("Stepper init failed: {e:?}");
                panic!("Stepper init failed: {e:?}");
            }
        };
    // let mut f = 1u32.Hz();
    let mut value = 0i8;
    loop {
        value = if value >= 5 { -5 } else { value + 1 };
        // stepper.drive(value)?;
        std::thread::sleep(Duration::from_millis(if value != 0 { 500 } else { 2000 }));
    }
}
