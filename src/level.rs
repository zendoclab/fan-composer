pub const MIN_LEVEL_DB: f32 = -60.0;
pub const MAX_LEVEL_DB: f32 = -6.0;
pub const DEFAULT_GAIN: f32 = 0.12589254; // -18 dB; always requires explicit Start
pub fn gain_to_db(gain: f32) -> f32 {
    if gain.is_finite() && gain > 0.0 {
        20.0 * gain.log10()
    } else {
        -120.0
    }
}
pub fn slider_db(gain: f32) -> f32 {
    gain_to_db(gain).clamp(MIN_LEVEL_DB, MAX_LEVEL_DB)
}
pub fn db_to_gain(db: f32) -> f32 {
    if !db.is_finite() || db <= MIN_LEVEL_DB {
        0.0
    } else {
        10.0_f32.powf(db.min(MAX_LEVEL_DB) / 20.0)
    }
}
/// Last numeric guard; smooth bounded transfer avoids hard-clipped plateaus.
pub fn limit(sample: f32, cap: f32) -> f32 {
    let cap = if cap.is_finite() {
        cap.clamp(0.001, 0.25)
    } else {
        0.05
    };
    if sample.is_finite() {
        cap * (sample / cap).tanh()
    } else {
        0.0
    }
}
