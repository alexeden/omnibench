use core::time::Duration;

use esp_idf_svc::hal::{
    gpio::OutputPin,
    rmt::{
        PinState, RmtChannel, Symbol, TxChannelDriver,
        config::{Loop, TransmitConfig, TxChannelConfig},
        encoder::CopyEncoder,
    },
    sys::EspError,
    units::Hertz,
};

pub struct FreqGen<'d> {
    driver: TxChannelDriver<'d>,
    encoder: CopyEncoder,
    /// Heap-allocated so its address stays stable even if `FreqGen` is moved.
    signal: Box<Symbol>,
    resolution: Hertz,
}

impl<'d> FreqGen<'d> {
    pub fn try_new(pin: impl OutputPin + 'd) -> Result<Self, EspError> {
        let channel_config = TxChannelConfig::default();
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
        if self.driver.is_enabled() {
            self.driver.disable()?;
        }

        let period = Duration::from_nanos(1_000_000_000u64 / u32::from(freq) as u64);
        *self.signal =
            Symbol::new_half_split(self.resolution, PinState::High, PinState::Low, period)?;

        let config = TransmitConfig {
            loop_count: Loop::Endless,
            ..TransmitConfig::default()
        };

        // SAFETY: `self.signal` is heap-allocated so its address is stable even if
        // `self` is moved. We stopped any in-progress transmission above before
        // mutating the signal.
        unsafe {
            self.driver.start_send(
                &mut self.encoder,
                core::slice::from_ref(&*self.signal),
                &config,
            )?;
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), EspError> {
        if self.driver.is_enabled() {
            self.driver.disable()?;
        }
        Ok(())
    }
}
