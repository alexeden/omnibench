use esp_idf_svc::hal::{
    adc::{AdcContConfig, AdcContDriver, AdcMeasurement, Attenuated},
    peripherals::Peripherals,
};
use log::info;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    let config = AdcContConfig {
        // resolution: Resolution::Resolution12Bit,
        ..AdcContConfig::default()
    };

    let adc_channel = Attenuated::db12(peripherals.pins.gpio8);
    let mut adc = AdcContDriver::new(peripherals.adc1, &config, adc_channel)?;
    adc.start()?;

    // Default to just read 100 measurements per each read
    let mut samples = [AdcMeasurement::default(); 100];
    let mut i = 0usize;
    let mut last_num_read = 0;
    let mut last_min = 0u16;
    let mut last_max = 0u16;
    loop {
        i = i.wrapping_add(1);
        if let Ok(num_read) = adc.read(&mut samples, 10) {
            let v = samples.into_iter().map(|s| s.data());
            last_num_read = num_read;
            last_min = v.clone().min().unwrap();
            last_max = v.clone().max().unwrap();
        }
        if i.is_multiple_of(100) {
            info!("{} samples: {:?}\t{:?}", last_num_read, last_min, last_max);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
