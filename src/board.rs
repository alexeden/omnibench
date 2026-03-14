/// Returns `(i2c_pwr, sda, scl)` for the board's I2C bus.
///
/// Selects GPIO22/GPIO20 on ESP32, or GPIO3/GPIO4 on ESP32-S3.
///
/// ```rust
/// let (i2c_pwr, sda, scl) = board_i2c_pins!(peripherals);
/// ```
#[macro_export]
macro_rules! board_i2c_pins {
    ($peripherals:expr) => {{
        #[cfg(not(esp32s3))]
        let pins = (
            $peripherals.pins.gpio2,
            $peripherals.pins.gpio22,
            $peripherals.pins.gpio20,
        );
        #[cfg(esp32s3)]
        let pins = (
            $peripherals.pins.gpio7,
            $peripherals.pins.gpio3,
            $peripherals.pins.gpio4,
        );
        pins
    }};
}

/// Returns `(AdcDriver, joy_gpio_pin, joy_sw_pin)` for the joystick ADC.
///
/// Selects ADC2/GPIO4 on ESP32, or ADC1/GPIO8 on ESP32-S3.
/// Must be called from a `?`-capable context.
///
/// ```rust
/// let (adc, joy_pin, sw_pin) = board_joy_adc!(peripherals);
/// ```
#[macro_export]
macro_rules! board_joy_adc {
    ($peripherals:expr) => {{
        #[cfg(not(esp32s3))]
        {
            let adc = esp_idf_svc::hal::adc::oneshot::AdcDriver::new($peripherals.adc2)?;
            let joy_pin = $peripherals.pins.gpio26; // A0
            let sw_pin = $peripherals.pins.gpio25; // A1
            (adc, joy_pin, sw_pin)
        }
        #[cfg(esp32s3)]
        {
            let adc = esp_idf_svc::hal::adc::oneshot::AdcDriver::new($peripherals.adc2)?;
            let joy_pin = $peripherals.pins.gpio18; // A0
            let sw_pin = $peripherals.pins.gpio17; // A1
            (adc, joy_pin, sw_pin)
        }
    }};
}

/// Returns `(dir, en, pul)` for the board's stepper motor.
///
/// EN - Green
/// DIR - Red
/// PUL - Blue
///
/// ```rust
/// let (dir, en, pul) = board_stepper_pins!(peripherals);
/// ``1
#[macro_export]
macro_rules! board_stepper_pins {
    ($peripherals:expr) => {{
        #[cfg(not(esp32s3))]
        let pins = (
            $peripherals.pins.gpio19, // MOSI
            $peripherals.pins.gpio5,  // SCK
            $peripherals.pins.gpio21, // MISO
        );
        #[cfg(esp32s3)]
        let pins = (
            $peripherals.pins.gpio35, // MOSI
            $peripherals.pins.gpio36, // SCK
            $peripherals.pins.gpio37, // MISO
        );
        pins
    }};
}

const JOY_ZERO_CLIP: u16 = 60;
const JOY_ZERO: u16 = 1635;
const JOY_MIN: u16 = 0;
const JOY_MAX: u16 = 3061;

/// Map an ADC millivolt value to a signed byte in the range -127..=127.
///
/// Run the `examples/adc_test.rs` example to get a sense of the mapping and the
/// JOY_* constants.
pub fn map_mv_to_i8(mv: u16) -> i8 {
    let mv = mv as f32;
    let zero = JOY_ZERO as f32;
    let clip = JOY_ZERO_CLIP as f32;
    let normalized = if mv < zero - clip {
        (mv - JOY_MIN as f32) / (zero - clip - JOY_MIN as f32) - 1.0 // -1.0..0.0
    } else if mv > zero + clip {
        (mv - (zero + clip)) / (JOY_MAX as f32 - (zero + clip)) // 0.0..1.0
    } else {
        0.0
    };
    (normalized * 127.0).round().clamp(-127.0, 128.0) as i8
}
