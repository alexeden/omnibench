use esp_idf_svc::hal::{
    // peripherals::Peripherals
    gpio::OutputPin,
    rmt::{
        PinState, Pulse, RmtChannel, TxChannelDriver,
        config::{Loop, TransmitConfig, TxChannelConfig},
    },
    sys::EspError,
};
// use esp_idf_svc::hal::rmt::TxRmtDriver;

pub struct FreqGen {
    driver: TxChannelDriver<'static>,
}

impl FreqGen {
    pub fn try_new(
        // channel: impl RmtChannel + Send + 'static,
        pin: impl OutputPin + Send + 'static,
    ) -> Result<Self, EspError> {
        let config = TxChannelConfig {
            // clock_divider: 32,
            // looping: Loop::None,
            ..Default::default()
        };
        let driver = TxRmtDriver::new(channel, pin, &config)?;
        Ok(Self { driver })
    }
}
