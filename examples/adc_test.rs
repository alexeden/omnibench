use esp_idf_svc::hal::{
    adc::{
        Resolution, attenuation,
        oneshot::{
            AdcChannelDriver,
            config::{AdcChannelConfig, Calibration},
        },
    },
    gpio::{PinDriver, Pull},
    peripherals::Peripherals,
};
use log::info;
use omnibench::board::map_mv_to_i8;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take()?;
    let (adc, joy_gpio, sw_pin) = omnibench::board_joy_adc!(peripherals);
    let sw = PinDriver::input(sw_pin, Pull::Down)?;
    let mut joy_pin = AdcChannelDriver::new(&adc, joy_gpio, &{
        #[cfg(not(esp32s3))]
        let cfg = AdcChannelConfig {
            attenuation: attenuation::DB_12,
            calibration: Calibration::Line,
            resolution: Resolution::Resolution12Bit,
        };
        #[cfg(esp32s3)]
        let cfg = AdcChannelConfig {
            attenuation: attenuation::DB_12,
            calibration: Calibration::Curve,
            resolution: Resolution::Resolution12Bit,
        };
        cfg
    })?;

    loop {
        let raw = adc.read_raw(&mut joy_pin)?;
        let mv = adc.raw_to_mv(&joy_pin, raw)?;
        let i8 = map_mv_to_i8(mv);
        info!("mv: {mv}, i8: {i8}, raw: {raw}, sw: {:?}", sw.get_level());
        std::thread::sleep(Duration::from_millis(200));
    }
}
