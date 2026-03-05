use core::time::Duration;
use embedded_hal::digital::{OutputPin, PinState};
use esp_idf_svc::hal::{
    gpio::{Output, OutputPin as EspOutputPin, PinDriver},
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
            resolution: 100u32.kHz().into(),
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
    pub fn set_freq(&mut self, freq: Hertz) -> Result<(), EspError> {
        info!(
            "Setting frequency to {}, resolution: {}",
            freq, self.resolution
        );
        if self.driver.is_enabled() {
            info!("Disabling driver");
            self.driver.disable()?;
        }

        let period = Duration::from_nanos(1_000_000_000u64 / u32::from(freq) as u64);
        info!("Period: {:?}", period);
        *self.signal =
            Symbol::new_half_split(self.resolution, RmtPinState::High, RmtPinState::Low, period)?;

        info!("Signal: {:?}", self.signal);
        let config = TransmitConfig {
            loop_count: Loop::Endless,
            ..TransmitConfig::default()
        };

        // SAFETY: `self.signal` is heap-allocated so its address is stable even if
        // `self` is moved. We stopped any in-progress transmission above before
        // mutating the signal.
        info!("Starting send");
        unsafe {
            self.driver.start_send(
                &mut self.encoder,
                core::slice::from_ref(&*self.signal),
                &config,
            )?;
        }

        info!("Send started");
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

    pub fn set_dir(&mut self, _dir: StepperDirection) -> Result<(), EspError> {
        // self.dir.set_state(dir.into())?;
        Ok(())
    }
}
