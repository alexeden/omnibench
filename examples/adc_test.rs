use esp_idf_svc::hal::{
    adc::{
        Resolution, attenuation,
        oneshot::{
            AdcChannelDriver, AdcDriver,
            config::{AdcChannelConfig, Calibration},
        },
    },
    peripherals::Peripherals,
};
use log::info;
use omnibench::joystick::map_mv_to_i8;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take()?;
    let adc = AdcDriver::new(peripherals.adc1)?;
    let mut joy_pin = AdcChannelDriver::new(
        &adc,
        peripherals.pins.gpio8, // A5
        &AdcChannelConfig {
            attenuation: attenuation::DB_12,
            calibration: Calibration::Curve,
            resolution: Resolution::Resolution12Bit,
        },
    )?;

    loop {
        let raw = adc.read_raw(&mut joy_pin)?;
        let mv = adc.raw_to_mv(&joy_pin, raw)?;
        let i8 = map_mv_to_i8(mv);
        info!("mv: {mv}, i8: {i8}, raw: {raw}");
        std::thread::sleep(Duration::from_millis(200));
    }
}
