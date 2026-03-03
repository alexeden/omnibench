use esp_idf_svc::hal::{
    adc::oneshot::{AdcChannelDriver, AdcDriver, config::AdcChannelConfig},
    peripherals::Peripherals,
};
use log::*;
use std::time::Duration;

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    let adc = AdcDriver::new(peripherals.adc2)?;
    let mut adc_pin = AdcChannelDriver::new(
        &adc,
        peripherals.pins.gpio11, // D11
        &AdcChannelConfig {
            // attenuation: attenuation::DB_12,
            ..Default::default()
        },
    )?;

    loop {
        info!("ADC value: {}", adc.read(&mut adc_pin)?);
        std::thread::sleep(Duration::from_millis(500));
    }
}
