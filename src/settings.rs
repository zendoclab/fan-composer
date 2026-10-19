use crate::{simulator::Scenario, synth::Mode};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub mode: Mode,
    pub volume: f32,
    pub max_peak: f32,
    pub adaptation: f32,
    pub duck: bool,
    pub periodic_calibration: bool,
    pub seed: u64,
    pub fft_size: usize,
    pub scenario: Scenario,
    pub input: Option<String>,
    pub output: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 3,
            mode: Mode::Harmonic,
            volume: crate::level::DEFAULT_GAIN * 2.0,
            max_peak: 0.08,
            adaptation: 0.25,
            duck: false,
            periodic_calibration: false,
            seed: 255,
            fft_size: 4096,
            scenario: Scenario::Harmonic,
            input: None,
            output: None,
        }
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FanComposer")
}
impl Settings {
    pub fn load() -> Self {
        fs::read(data_dir().join("settings.json"))
            .ok()
            .map(|bytes| Self::from_json(&bytes))
            .unwrap_or_default()
    }
    pub fn from_json(bytes: &[u8]) -> Self {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return Self::default();
        };
        if !value.is_object() {
            return Self::default();
        }
        let legacy = value
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            < 3
            && value.get("volume").is_some();
        let mut s: Self = serde_json::from_value(value).unwrap_or_default();
        // Old volume represented a fixed reference gain. Preserve that starting level,
        // adding 6 dB of visible maximum headroom for the always-on controller.
        if legacy {
            s.volume *= 2.0;
        }
        s.schema_version = 3;
        s.volume = if s.volume.is_finite() {
            s.volume
                .clamp(0.0, crate::level::db_to_gain(crate::level::MAX_LEVEL_DB))
        } else {
            crate::level::DEFAULT_GAIN
        };
        s.max_peak = if s.max_peak.is_finite() {
            s.max_peak.clamp(0.005, 0.15)
        } else {
            0.08
        };
        s.adaptation = if s.adaptation.is_finite() {
            s.adaptation.clamp(0.05, 1.0)
        } else {
            0.25
        };
        if !matches!(s.fft_size, 2048 | 4096) {
            s.fft_size = 4096;
        }
        s
    }
    pub fn save(&self) -> Result<(), String> {
        fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let temp = data_dir().join("settings.tmp");
        fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        fs::rename(temp, data_dir().join("settings.json")).map_err(|e| e.to_string())
    }
    pub fn feedback(
        &self,
        rating: i32,
        features: &crate::analysis::Features,
    ) -> Result<(), String> {
        use std::io::Write;
        fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
        let entry = serde_json::json!({"unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),"rating":rating.clamp(-1,1),"settings":self,"features":features,"raw_audio_saved":false});
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(data_dir().join("feedback.jsonl"))
            .map_err(|e| e.to_string())?;
        writeln!(file, "{entry}").map_err(|e| e.to_string())
    }
}
