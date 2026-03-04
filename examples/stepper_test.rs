use esp_idf_svc::hal::peripherals::Peripherals;
use log::*;
use omnibench::stepper::FreqGen;
use std::time::Duration;

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    let _freq_gen = FreqGen::try_new(peripherals.pins.gpio0)?;
    info!("FreqGen initialized");
    // let rmt = RmtDriver::new(peripherals.rmt.channel0)?;
    // let mut adc_pin = AdcChannelDriver::new(
    //     &adc,
    //     peripherals.pins.gpio11, // D11
    //     &AdcChannelConfig {
    //         // attenuation: attenuation::DB_12,
    //         ..Default::default()
    //     },
    // )?;

    loop {
        std::thread::sleep(Duration::from_millis(500));
    }
}
