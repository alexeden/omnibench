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
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // gpio14 is A4

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
        info!("raw: {}, mv: {}, i8: {}", raw, mv, i8);
        std::thread::sleep(Duration::from_millis(200));
    }
}

const JOY_ZERO: u16 = 1565;
const JOY_MIN: u16 = 0;
const JOY_MAX: u16 = 3061;

fn map_mv_to_i8(mv: u16) -> i8 {
    let mv = mv as f32;
    let normalized = if mv < JOY_ZERO as f32 {
        (mv - JOY_MIN as f32) / (JOY_ZERO as f32 - JOY_MIN as f32) - 1.0 // -1.0..0.0
    } else {
        (mv - JOY_ZERO as f32) / (JOY_MAX as f32 - JOY_ZERO as f32) // 0.0..1.0
    };
    (normalized * 127.0).round().clamp(-127.0, 128.0) as i8
}
