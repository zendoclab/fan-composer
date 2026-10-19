use crate::{
    analysis::Features,
    openvino_native::{Session, asset_paths},
    synth::{Mode, Parameters},
};
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Serialize)]
pub struct AiStatus {
    pub actual_backend: String,
    pub execution_device: String,
    pub available_devices: Vec<String>,
    pub model: String,
    pub fallback_reason: Option<String>,
    pub compile_ms: f64,
    pub last_inference_ms: f64,
    pub npu_decisions: u64,
    pub cpu_decisions: u64,
    pub rule_decisions: u64,
}
impl Default for AiStatus {
    fn default() -> Self {
        Self {
            actual_backend: "Initializing NPU".into(),
            execution_device: String::new(),
            available_devices: vec![],
            model: "teacher-distilled-policy-v1".into(),
            fallback_reason: None,
            compile_ms: 0.0,
            last_inference_ms: 0.0,
            npu_decisions: 0,
            cpu_decisions: 0,
            rule_decisions: 0,
        }
    }
}
pub struct Engine {
    session: Option<Session>,
    pub status: AiStatus,
}
impl Engine {
    pub fn new(force_cpu: bool) -> Self {
        let mut e = Self::for_device_order(if force_cpu { &["CPU"] } else { &["NPU", "CPU"] });
        if force_cpu {
            e.status.fallback_reason = Some("CPU forced for explicit fallback validation".into());
        }
        e
    }
    pub fn for_device_order(order: &[&str]) -> Self {
        let mut e = Self {
            session: None,
            status: AiStatus::default(),
        };
        for device in order {
            e.load_device(device);
            if e.session.is_some() {
                break;
            }
        }
        e
    }
    fn load_device(&mut self, device: &str) {
        let (runtime, model) = asset_paths();
        match Session::load(&runtime, &model, device) {
            Ok(s) => {
                self.status.actual_backend = if device == "NPU" {
                    "NPU"
                } else {
                    "CPU (OpenVINO)"
                }
                .into();
                self.status.execution_device = s.execution_device.clone();
                self.status.available_devices = s.available_devices.clone();
                self.status.compile_ms = s.compile_ms;
                self.session = Some(s);
            }
            Err(error) => {
                let previous = self.status.fallback_reason.take();
                self.status.fallback_reason = Some(if let Some(previous) = previous {
                    format!("{previous}; {device}: {error}")
                } else {
                    format!("{device}: {error}")
                });
                self.status.actual_backend = "CPU (rules fallback)".into();
                self.status.execution_device = "CPU".into();
                self.session = None;
            }
        }
    }
    pub fn infer(&mut self, input: &[f32; 16]) -> [f32; 6] {
        let began = Instant::now();
        let output = self.session.as_mut().map(|s| s.infer(input));
        let result = match output {
            Some(Ok(values)) => {
                if self.status.actual_backend == "NPU" {
                    self.status.npu_decisions += 1;
                } else {
                    self.status.cpu_decisions += 1;
                }
                values
            }
            Some(Err(error)) => {
                let was_npu = self.status.actual_backend == "NPU";
                self.status.fallback_reason = Some(format!(
                    "{} inference failed: {error}",
                    self.status.actual_backend
                ));
                self.session = None;
                if was_npu {
                    self.load_device("CPU");
                } else {
                    self.status.actual_backend = "CPU (rules fallback)".into();
                    self.status.execution_device = "CPU".into();
                }
                if self.session.is_some() {
                    return self.infer(input);
                }
                self.status.rule_decisions += 1;
                teacher(input)
            }
            None => {
                self.status.rule_decisions += 1;
                teacher(input)
            }
        };
        self.status.last_inference_ms = began.elapsed().as_secs_f64() * 1000.0;
        result
    }
}
pub fn teacher(x: &[f32; 16]) -> [f32; 6] {
    [
        0.25 + 0.30 * x[3] + 0.12 * x[4] + 0.08 * x[14] - 0.10 * x[15] + 0.12 * x[11],
        0.20 + 0.28 * x[2] - 0.13 * x[4] - 0.10 * x[6] + 0.14 * x[14] + 0.12 * x[12],
        0.32 + 0.30 * x[2] + 0.12 * x[8] - 0.15 * x[4] + 0.05 * x[10],
        0.30 + 0.18 * x[9] - 0.10 * x[7] + 0.22 * x[13] + 0.08 * x[14],
        0.62 - 0.16 * x[1] + 0.12 * x[4] + 0.06 * x[14],
        0.60 + 0.20 * x[14] - 0.15 * x[1] + 0.10 * x[5],
    ]
    .map(|v| v.clamp(0.05, 0.95))
}
fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
pub fn encode(f: &Features, p: Parameters, time: f64) -> [f32; 16] {
    let angle = time as f32 * std::f32::consts::TAU / 90.0;
    [
        unit((f.fan_hz.max(65.0) / 65.0).log2() / 3.0),
        unit((f.dbfs + 90.0) / 60.0),
        unit(f.tonality),
        unit(f.centroid / 6000.0),
        unit(f.flatness),
        unit((20.0 * f.broadband.max(1e-6).log10() + 90.0) / 60.0),
        unit(f.pitch_change.abs() / 1200.0),
        unit(f.level_change.abs() / 60.0),
        unit(f.confidence),
        angle.sin() * 0.5 + 0.5,
        angle.cos() * 0.5 + 0.5,
        unit((p.brightness - 0.15) / 0.5),
        unit((p.density - 0.15) / 0.25),
        unit(p.motion - 0.5),
        match p.mode {
            Mode::Harmonic => 0.0,
            Mode::Nature => 0.5,
            Mode::Generative => 1.0,
        },
        unit(p.preference * 0.5 + 0.5),
    ]
}
fn apply(mut p: Parameters, y: [f32; 6]) -> Parameters {
    // Conservative decision smoothing; audio thread adds continuous parameter smoothing.
    p.brightness += 0.18 * ((0.15 + 0.5 * y[0]) - p.brightness);
    p.density += 0.18 * ((0.15 + 0.25 * y[1]) - p.density);
    p.overtones += 0.18 * ((0.6 + 0.8 * y[2]) - p.overtones);
    p.motion += 0.18 * ((0.5 + y[3]) - p.motion);
    p.wet += 0.18 * ((0.05 + 0.2 * y[4]) - p.wet);
    p.stereo += 0.18 * ((0.4 + 0.6 * y[5]) - p.stereo);
    p
}
#[derive(Clone, Copy)]
pub struct Job {
    pub input: [f32; 16],
    pub parameters: Parameters,
}
pub struct ResultPacket {
    pub parameters: Parameters,
    pub status: AiStatus,
}
pub struct PolicyWorker {
    pub requests: rtrb::Producer<Job>,
    pub results: rtrb::Consumer<ResultPacket>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl PolicyWorker {
    pub fn start() -> Result<Self, String> {
        let (tx, mut rx) = rtrb::RingBuffer::<Job>::new(2);
        let (mut result_tx, result_rx) = rtrb::RingBuffer::<ResultPacket>::new(4);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("fan-npu-policy".into())
            .spawn(move || {
                let mut engine = Engine::new(false);
                while !worker_stop.load(Ordering::Acquire) {
                    if let Ok(job) = rx.pop() {
                        let values = engine.infer(&job.input);
                        let packet = ResultPacket {
                            parameters: apply(job.parameters, values),
                            status: engine.status.clone(),
                        };
                        let _ = result_tx.push(packet);
                    } else {
                        thread::park_timeout(Duration::from_millis(10));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            requests: tx,
            results: result_rx,
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for PolicyWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
