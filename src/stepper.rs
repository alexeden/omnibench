use crate::freq_gen::FreqGen;
use core::time::Duration;
use esp_idf_svc::hal::{
    gpio::{Level, Output, OutputPin as EspOutputPin, PinDriver},
    sys::EspError,
};
use log::*;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub enum StepperDirection {
    Forward,
    Reverse,
}

impl From<StepperDirection> for Level {
    fn from(value: StepperDirection) -> Self {
        match value {
            StepperDirection::Reverse => Level::Low,
            StepperDirection::Forward => Level::High,
        }
    }
}

// At 1 MHz RMT resolution, the 15-bit duration field tops out at 32767 ticks
// (32.77 ms per half-period), giving a minimum frequency of ~15 Hz.
const MIN_SPEED_HZ: f32 = 20.0;
const MAX_SPEED_HZ: f32 = 20_000.0;

/// Acceleration / deceleration ramp configuration.
#[derive(Clone, Debug)]
pub struct RampConfig {
    /// Hz gained per second when speed magnitude is increasing.
    pub accel_hz_per_s: f32,
    /// Hz lost per second when speed magnitude is decreasing (or reversing).
    pub decel_hz_per_s: f32,
    /// How long after the last joystick input before the target is forced to
    /// zero.  Acts as a fail-safe if the BLE connection drops.
    pub input_timeout: Duration,
}

impl Default for RampConfig {
    fn default() -> Self {
        Self {
            accel_hz_per_s: MAX_SPEED_HZ / 2.,
            decel_hz_per_s: MAX_SPEED_HZ,
            input_timeout: Duration::from_millis(500),
        }
    }
}

pub struct Stepper<'d> {
    dir: PinDriver<'d, Output>,
    en: PinDriver<'d, Output>,
    pulse: FreqGen<'d>,
    ramp: RampConfig,
    /// Signed current speed in Hz (positive = Forward).
    current_hz: f32,
    /// Signed target speed set by the most recent joystick input.
    target_hz: f32,
    /// When `set_target` was last called (i.e. when fresh input last arrived).
    last_input: Instant,
    /// The speed last written to hardware, used to skip redundant updates.
    last_applied_hz: f32,
}

impl<'d> Stepper<'d> {
    pub fn try_new(
        dir: impl EspOutputPin + 'd,
        en: impl EspOutputPin + 'd,
        pul: impl EspOutputPin + 'd,
        ramp: RampConfig,
    ) -> Result<Self, EspError> {
        let pulse = FreqGen::try_new(pul)?;
        let dir = PinDriver::output(dir)?;
        let en = PinDriver::output(en)?;
        Ok(Self {
            pulse,
            dir,
            en,
            ramp,
            current_hz: 0.0,
            target_hz: 0.0,
            last_input: Instant::now(),
            last_applied_hz: 0.0,
        })
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

    /// Direct drive bypassing the ramp.  Kept for use in tests.
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

    /// Advance the ramp by `dt` and apply the result to the hardware.
    ///
    /// Pass `new_target = Some(value)` when a fresh joystick value arrived
    /// this tick; pass `None` when no new input has come in since the last
    /// call.  The ramp's `input_timeout` will force the target to zero if
    /// `None` keeps arriving past the deadline.
    pub fn tick(&mut self, new_target: Option<i8>, dt: Duration) -> Result<(), EspError> {
        if let Some(value) = new_target {
            self.target_hz = joy_to_hz_signed(value);
            self.last_input = Instant::now();
        }

        let effective_target = if self.last_input.elapsed() > self.ramp.input_timeout {
            0.0
        } else {
            self.target_hz
        };

        // Advance current_hz toward effective_target at the appropriate rate.
        let diff = effective_target - self.current_hz;
        if diff != 0.0 {
            // Accelerating: magnitude is increasing and we're not reversing.
            let is_accel = (effective_target.abs() > self.current_hz.abs())
                && (self.current_hz == 0.0
                    || effective_target.signum() == self.current_hz.signum());
            let rate = if is_accel {
                self.ramp.accel_hz_per_s
            } else {
                self.ramp.decel_hz_per_s
            };
            let step = rate * dt.as_secs_f32();
            self.current_hz = if diff.abs() <= step {
                effective_target
            } else {
                self.current_hz + diff.signum() * step
            };
        }

        // Skip the hardware update if nothing meaningful changed.
        let now_running = self.current_hz.abs() >= MIN_SPEED_HZ;
        let was_running = self.last_applied_hz.abs() >= MIN_SPEED_HZ;
        if !now_running && !was_running {
            return Ok(());
        }
        if now_running == was_running && (self.current_hz - self.last_applied_hz).abs() < 0.5 {
            return Ok(());
        }
        self.last_applied_hz = self.current_hz;

        if !now_running {
            self.pulse.stop()?;
            self.disable()?;
        } else {
            let dir = if self.current_hz > 0.0 {
                StepperDirection::Forward
            } else {
                StepperDirection::Reverse
            };
            self.set_dir(dir)?;
            self.pulse.set_freq(self.current_hz.abs() as u32)?;
            self.enable()?;
            info!("Stepper ramp: {:.0} Hz", self.current_hz);
        }
        Ok(())
    }
}

fn map_joy_to_hz(abs_value: u8) -> u32 {
    let t = abs_value as f32 / 127.0;
    (MIN_SPEED_HZ + t * (MAX_SPEED_HZ - MIN_SPEED_HZ)) as u32
}

fn joy_to_hz_signed(value: i8) -> f32 {
    if value == 0 {
        0.0
    } else {
        let sign = if value > 0 { 1.0f32 } else { -1.0f32 };
        sign * map_joy_to_hz(value.unsigned_abs()) as f32
    }
}
