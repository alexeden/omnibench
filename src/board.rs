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

/// Returns `(AdcDriver, joy_gpio_pin)` for the joystick ADC.
///
/// Selects ADC2/GPIO4 on ESP32, or ADC1/GPIO8 on ESP32-S3.
/// Must be called from a `?`-capable context.
///
/// ```rust
/// let (adc, joy_pin) = board_joy_adc!(peripherals);
/// ```
#[macro_export]
macro_rules! board_joy_adc {
    ($peripherals:expr) => {{
        #[cfg(not(esp32s3))]
        {
            let adc = esp_idf_svc::hal::adc::oneshot::AdcDriver::new($peripherals.adc2)?;
            let pin = $peripherals.pins.gpio26; // A0
            (adc, pin)
        }
        #[cfg(esp32s3)]
        {
            let adc = esp_idf_svc::hal::adc::oneshot::AdcDriver::new($peripherals.adc2)?;
            let pin = $peripherals.pins.gpio18; // A0
            (adc, pin)
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
