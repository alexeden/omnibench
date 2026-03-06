use core::time::Duration;
use embedded_hal::digital::PinState;
use esp_idf_svc::hal::{
    gpio::{Level, Output, OutputPin as EspOutputPin, PinDriver},
    rmt::{
        PinState as RmtPinState, RmtChannel, Symbol, TxChannelDriver,
        config::{Loop, TransmitConfig, TxChannelConfig},
        encoder::CopyEncoder,
    },
    sys::EspError,
    units::{FromValueType, Hertz},
};
use log::*;

pub struct FreqGen<'d> {
    driver: TxChannelDriver<'d>,
    encoder: CopyEncoder,
    /// Heap-allocated so its address stays stable even if `FreqGen` is moved.
    signal: Box<Symbol>,
    resolution: Hertz,
}

impl<'d> FreqGen<'d> {
    pub fn try_new(pin: impl EspOutputPin + 'd) -> Result<Self, EspError> {
        let channel_config = TxChannelConfig {
            // 80 MHz APB / 1 MHz = div 80, which is within the [1, 256] hw limit.
            resolution: 1u32.MHz().into(),
            ..TxChannelConfig::default()
        };
        let resolution = channel_config.resolution;
        let driver = TxChannelDriver::new(pin, &channel_config)?;
        let encoder = CopyEncoder::new()?;
        Ok(Self {
            driver,
            encoder,
            signal: Box::new(Symbol::default()),
            resolution,
        })
    }

    /// Start (or update) the continuous square-wave output at the given
    /// frequency.
    ///
    /// Safe to call while already running — the previous transmission is
    /// stopped first.
    pub fn set_freq<F: Into<Hertz>>(&mut self, freq: F) -> Result<(), EspError> {
        let freq: Hertz = freq.into();
        if self.driver.is_enabled() {
            // info!("Disabling driver");
            self.driver.disable()?;
        }

        let period = Duration::from_nanos(1_000_000_000u64 / u32::from(freq) as u64);
        // info!("Period: {:?}", period);
        *self.signal =
            Symbol::new_half_split(self.resolution, RmtPinState::High, RmtPinState::Low, period)?;

        // info!("Signal: {:?}", self.signal);
        let config = TransmitConfig {
            loop_count: Loop::Endless,
            ..TransmitConfig::default()
        };

        // SAFETY: `self.signal` is heap-allocated so its address is stable even if
        // `self` is moved. We stopped any in-progress transmission above before
        // mutating the signal.
        // info!("Starting send");
        unsafe {
            self.driver.start_send(
                &mut self.encoder,
                core::slice::from_ref(&*self.signal),
                &config,
            )?;
        }
        info!("Frequency set to {freq}");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), EspError> {
        if self.driver.is_enabled() {
            self.driver.disable()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StepperDirection {
    Forward,
    Reverse,
}

impl From<StepperDirection> for PinState {
    fn from(value: StepperDirection) -> Self {
        match value {
            StepperDirection::Reverse => PinState::Low,
            StepperDirection::Forward => PinState::High,
        }
    }
}

impl From<StepperDirection> for Level {
    fn from(value: StepperDirection) -> Self {
        match value {
            StepperDirection::Reverse => Level::Low,
            StepperDirection::Forward => Level::High,
        }
    }
}

pub struct Stepper<'d> {
    dir: PinDriver<'d, Output>,
    en: PinDriver<'d, Output>,
    pulse: FreqGen<'d>,
}

impl<'d> Stepper<'d> {
    pub fn try_new(
        dir: impl EspOutputPin + 'd,
        en: impl EspOutputPin + 'd,
        pul: impl EspOutputPin + 'd,
    ) -> Result<Self, EspError> {
        let pulse = FreqGen::try_new(pul)?;
        let dir = PinDriver::output(dir)?;
        let en = PinDriver::output(en)?;
        Ok(Self { pulse, dir, en })
    }

    pub fn disable(&mut self) -> Result<&mut Self, EspError> {
        self.en.set_high()?;
        Ok(self)
    }

    pub fn enable(&mut self) -> Result<&mut Self, EspError> {
        self.en.set_low()?;
        Ok(self)
    }

    pub fn set_dir(&mut self, dir: StepperDirection) -> Result<(), EspError> {
        self.dir.set_level(dir.into())?;
        Ok(())
    }

    /// Drive the stepper from a signed joystick value (-127..=128).
    /// Zero disables; sign sets direction; magnitude sets speed.
    pub fn drive(&mut self, value: i8) -> Result<(), EspError> {
        if value == 0 {
            self.pulse.stop()?;
            self.disable()?;
        } else {
            info!("Driving stepper: {value}");
            let dir = if value > 0 {
                StepperDirection::Forward
            } else {
                StepperDirection::Reverse
            };
            self.set_dir(dir)?;
            self.pulse.set_freq(map_joy_to_hz(value.unsigned_abs()))?;
            self.enable()?;
        }
        Ok(())
    }
}

// At 1 MHz RMT resolution, the 15-bit duration field tops out at 32767 ticks
// (32.77 ms per half-period), giving a minimum frequency of ~15 Hz.
const MIN_SPEED_HZ: u32 = 20;
const MAX_SPEED_HZ: u32 = 400;

fn map_joy_to_hz(abs_value: u8) -> u32 {
    let t = abs_value as f32 / 127.0;
    (MIN_SPEED_HZ as f32 + t * (MAX_SPEED_HZ - MIN_SPEED_HZ) as f32) as u32
}
