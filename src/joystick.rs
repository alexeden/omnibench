const JOY_ZERO_CLIP: u16 = 50;
const JOY_ZERO: u16 = 1563;
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
