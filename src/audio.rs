use crate::{
    analysis::{Analyzer, Features},
    settings::Settings,
    simulator::FanSimulator,
    synth::{Mode, Parameters, Synth},
};
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use rtrb::{Consumer, Producer, RingBuffer};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub struct AtomicFloat(AtomicU32);
impl AtomicFloat {
    pub fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
}
pub struct Controls {
    pub stop: AtomicBool,
    pub test_request: AtomicBool,
    pub test_active: AtomicBool,
    pub fault: AtomicU32,
    pub volume: AtomicFloat,
    pub max_peak: AtomicFloat,
    pub adaptation: AtomicFloat,
    pub mode: AtomicU32,
    pub duck: AtomicBool,
    pub calibrate: AtomicBool,
    pub periodic_calibration: AtomicBool,
    pub quiet: AtomicBool,
    pub attenuation: AtomicFloat,
    pub adaptive_gain: AtomicFloat,
    pub reference_dropped: AtomicU64,
    pub input_frames: AtomicU64,
    pub output_frames: AtomicU64,
    pub input_dropped: AtomicU64,
    pub output_peak: AtomicFloat,
    pub output_rms: AtomicFloat,
    pub callback_max_us: AtomicU64,
    pub callback_late: AtomicU64,
    pub input_error_kind: AtomicU32,
    pub output_error_kind: AtomicU32,
    pub xruns: AtomicU64,
    pub realtime_warnings: AtomicU64,
}
impl Controls {
    fn new(s: &Settings) -> Self {
        Self {
            stop: AtomicBool::new(false),
            test_request: AtomicBool::new(false),
            test_active: AtomicBool::new(false),
            fault: AtomicU32::new(0),
            volume: AtomicFloat::new(s.volume),
            max_peak: AtomicFloat::new(s.max_peak),
            adaptation: AtomicFloat::new(s.adaptation),
            mode: AtomicU32::new(s.mode as u32),
            duck: AtomicBool::new(s.duck),
            calibrate: AtomicBool::new(false),
            periodic_calibration: AtomicBool::new(s.periodic_calibration),
            quiet: AtomicBool::new(true),
            attenuation: AtomicFloat::new(1.0),
            adaptive_gain: AtomicFloat::new(crate::auto_volume::gain_from_db(
                crate::auto_volume::INITIAL_DB,
            )),
            reference_dropped: AtomicU64::new(0),
            input_frames: AtomicU64::new(0),
            output_frames: AtomicU64::new(0),
            input_dropped: AtomicU64::new(0),
            output_peak: AtomicFloat::new(0.0),
            output_rms: AtomicFloat::new(0.0),
            callback_max_us: AtomicU64::new(0),
            callback_late: AtomicU64::new(0),
            input_error_kind: AtomicU32::new(0),
            output_error_kind: AtomicU32::new(0),
            xruns: AtomicU64::new(0),
            realtime_warnings: AtomicU64::new(0),
        }
    }
    pub fn apply(&self, s: &Settings) {
        self.volume.set(s.volume);
        self.max_peak.set(s.max_peak);
        self.adaptation.set(s.adaptation);
        self.mode.store(s.mode as u32, Ordering::Relaxed);
        self.duck.store(s.duck, Ordering::Relaxed);
        self.periodic_calibration
            .store(s.periodic_calibration, Ordering::Relaxed);
    }
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct Telemetry {
    pub features: Features,
    pub root_hz: f32,
    pub quiet: bool,
    pub frozen: bool,
    pub input_rate: u32,
    pub output_rate: u32,
    pub status: String,
    pub backend: String,
    pub ai: crate::policy::AiStatus,
    pub auto_volume: crate::auto_volume::VolumeStatus,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceChoice {
    pub id: String,
    pub name: String,
}
impl std::fmt::Display for DeviceChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
pub fn choices(input: bool) -> Vec<DeviceChoice> {
    let host = cpal::default_host();
    let mut result = vec![DeviceChoice {
        id: String::new(),
        name: "Windows default".into(),
    }];
    if let Ok(devices) = host.devices() {
        for d in devices {
            let supported = if input {
                d.supports_input()
            } else {
                d.supports_output()
            };
            if supported && let Ok(id) = d.id() {
                result.push(DeviceChoice {
                    id: id.to_string(),
                    name: d.to_string(),
                });
            }
        }
    }
    result
}
fn device(input: bool, id: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    if let Some(id) = id.filter(|s| !s.is_empty()) {
        return host
            .devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.id().map(|x| x.to_string() == id).unwrap_or(false))
            .ok_or_else(|| "Selected device disconnected; select a device again".into());
    }
    (if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    })
    .ok_or_else(|| "No default audio device".into())
}
fn config(d: &cpal::Device, input: bool) -> Result<cpal::SupportedStreamConfig, String> {
    let fallback = if input {
        d.default_input_config()
    } else {
        d.default_output_config()
    }
    .map_err(|e| e.to_string())?;
    let ranges: Vec<_> = if input {
        d.supported_input_configs().map(|i| i.collect())
    } else {
        d.supported_output_configs().map(|i| i.collect())
    }
    .unwrap_or_default();
    Ok(ranges
        .into_iter()
        .find(|r| {
            r.sample_format() == SampleFormat::F32
                && r.min_sample_rate() <= 48000
                && r.max_sample_rate() >= 48000
                && r.channels() <= 2
        })
        .map(|r| r.with_sample_rate(48000))
        .unwrap_or(fallback))
}

pub struct AudioSession {
    pub controls: Arc<Controls>,
    pub telemetry: Arc<Mutex<Telemetry>>,
    input: Option<cpal::Stream>,
    output: Option<cpal::Stream>,
    worker: Option<thread::JoinHandle<()>>,
}
impl AudioSession {
    pub fn start(s: &Settings, synthetic: bool, output_enabled: bool) -> Result<Self, String> {
        let controls = Arc::new(Controls::new(s));
        let (producer, consumer) = RingBuffer::<f32>::new(65536);
        let (params_tx, params_rx) = RingBuffer::<Parameters>::new(16);
        let (reference_tx, reference_rx) =
            RingBuffer::<crate::auto_volume::ReferenceFrame>::new(65536);
        let mut input = None;
        let mut output = None;
        let input_rate = if synthetic {
            48000
        } else {
            let d = device(true, s.input.as_deref())?;
            let cfg = config(&d, true)?;
            let rate = cfg.sample_rate();
            input = Some(make_input(&d, cfg, producer, controls.clone())?);
            rate
        };
        let output_rate = if output_enabled {
            let d = device(false, s.output.as_deref())?;
            let cfg = config(&d, false)?;
            let rate = cfg.sample_rate();
            output = Some(make_output(
                &d,
                cfg,
                params_rx,
                reference_tx,
                controls.clone(),
                s.seed,
            )?);
            rate
        } else {
            48000
        };
        let telemetry = Arc::new(Mutex::new(Telemetry {
            input_rate,
            output_rate,
            status: "Silent calibration".into(),
            backend: if output_enabled {
                "Initializing NPU policy"
            } else {
                "Analysis only"
            }
            .into(),
            ..Default::default()
        }));
        // Start physical streams only after all fallible construction succeeds.
        if let Some(stream) = &input {
            stream.play().map_err(|e| e.to_string())?;
        }
        if let Some(stream) = &output {
            stream.play().map_err(|e| e.to_string())?;
        }
        let c = controls.clone();
        let t = telemetry.clone();
        let settings = s.clone();
        let worker = thread::Builder::new()
            .name("fan-analysis".into())
            .spawn(move || {
                analyze_worker(
                    consumer,
                    reference_rx,
                    params_tx,
                    c,
                    t,
                    WorkerConfig {
                        settings,
                        synthetic,
                        output_enabled,
                        rate: input_rate,
                        output_rate,
                    },
                )
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            controls,
            telemetry,
            input,
            output,
            worker: Some(worker),
        })
    }
    pub fn speaker_test(s: &Settings) -> Result<Self, String> {
        let controls = Arc::new(Controls::new(s));
        let (_tx, rx) = RingBuffer::<Parameters>::new(16);
        let d = device(false, s.output.as_deref())?;
        let cfg = config(&d, false)?;
        let output_rate = cfg.sample_rate();
        let (reference_tx, _reference_rx) =
            RingBuffer::<crate::auto_volume::ReferenceFrame>::new(1);
        let output = make_output(&d, cfg, rx, reference_tx, controls.clone(), s.seed)?;
        controls.test_request.store(true, Ordering::Release);
        output.play().map_err(|e| e.to_string())?;
        let telemetry = Arc::new(Mutex::new(Telemetry {
            output_rate,
            status: "500 Hz speaker test; microphone OFF".into(),
            backend: "Rust test oscillator".into(),
            ..Default::default()
        }));
        Ok(Self {
            controls,
            telemetry,
            input: None,
            output: Some(output),
            worker: None,
        })
    }
    pub fn snapshot(&self) -> Telemetry {
        self.telemetry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
impl Drop for AudioSession {
    fn drop(&mut self) {
        self.controls.stop.store(true, Ordering::Release);
        // Outside callbacks: permit a short smooth release before destroying streams.
        thread::sleep(Duration::from_millis(90));
        self.output.take();
        self.input.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn make_input(
    d: &cpal::Device,
    cfg: cpal::SupportedStreamConfig,
    p: Producer<f32>,
    c: Arc<Controls>,
) -> Result<cpal::Stream, String> {
    match cfg.sample_format() {
        SampleFormat::F32 => input_stream::<f32>(d, cfg.config(), p, c),
        SampleFormat::F64 => input_stream::<f64>(d, cfg.config(), p, c),
        SampleFormat::I16 => input_stream::<i16>(d, cfg.config(), p, c),
        SampleFormat::I32 => input_stream::<i32>(d, cfg.config(), p, c),
        SampleFormat::U16 => input_stream::<u16>(d, cfg.config(), p, c),
        other => Err(format!("Unsupported capture format: {other}")),
    }
}
fn input_stream<T>(
    d: &cpal::Device,
    cfg: cpal::StreamConfig,
    mut p: Producer<f32>,
    c: Arc<Controls>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = cfg.channels as usize;
    let err = c.clone();
    d.build_input_stream(
        cfg,
        move |data: &[T], _| {
            if c.stop.load(Ordering::Relaxed) {
                return;
            }
            let mut dropped = 0;
            for frame in data.chunks_exact(channels) {
                let sample =
                    frame.iter().map(|&v| f32::from_sample(v)).sum::<f32>() / channels as f32;
                if !sample.is_finite() {
                    c.fault.store(3, Ordering::Relaxed);
                    continue;
                }
                if p.push(sample).is_err() {
                    dropped += 1;
                }
            }
            c.input_frames
                .fetch_add((data.len() / channels) as u64, Ordering::Relaxed);
            c.input_dropped.fetch_add(dropped, Ordering::Relaxed);
        },
        move |e| record_stream_error(&err, &e, true),
        Some(Duration::from_secs(3)),
    )
    .map_err(|e| e.to_string())
}
fn make_output(
    d: &cpal::Device,
    cfg: cpal::SupportedStreamConfig,
    p: Consumer<Parameters>,
    reference: Producer<crate::auto_volume::ReferenceFrame>,
    c: Arc<Controls>,
    seed: u64,
) -> Result<cpal::Stream, String> {
    match cfg.sample_format() {
        SampleFormat::F32 => output_stream::<f32>(d, cfg.config(), p, reference, c, seed),
        SampleFormat::F64 => output_stream::<f64>(d, cfg.config(), p, reference, c, seed),
        SampleFormat::I16 => output_stream::<i16>(d, cfg.config(), p, reference, c, seed),
        SampleFormat::I32 => output_stream::<i32>(d, cfg.config(), p, reference, c, seed),
        SampleFormat::U16 => output_stream::<u16>(d, cfg.config(), p, reference, c, seed),
        other => Err(format!("Unsupported playback format: {other}")),
    }
}
fn output_stream<T>(
    d: &cpal::Device,
    cfg: cpal::StreamConfig,
    mut p: Consumer<Parameters>,
    mut reference: Producer<crate::auto_volume::ReferenceFrame>,
    c: Arc<Controls>,
    seed: u64,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = cfg.channels as usize;
    let rate = cfg.sample_rate as f64;
    let mut synth = Synth::new(cfg.sample_rate as f32, seed);
    let mut speaker_test = crate::speaker_test::SpeakerTest::new(cfg.sample_rate as f32);
    let err = c.clone();
    d.build_output_stream(
        cfg,
        move |data: &mut [T], _| {
            let began = Instant::now();
            // Bounded queue drain: the producer never runs faster than STFT frames.
            for _ in 0..16 {
                match p.pop() {
                    Ok(v) => synth.set_parameters(v),
                    Err(_) => break,
                }
            }
            if c.test_request.swap(false, Ordering::AcqRel) {
                speaker_test.trigger();
            }
            if c.stop.load(Ordering::Acquire) {
                speaker_test.stop();
            }
            let fault = c.fault.load(Ordering::Acquire) != 0;
            let mute = c.stop.load(Ordering::Acquire) || c.quiet.load(Ordering::Acquire);
            let gain = if mute || fault {
                0.0
            } else {
                crate::auto_volume::limited_gain(
                    c.volume.get(),
                    c.adaptive_gain.get(),
                    c.attenuation.get(),
                )
            };
            let cap = c.max_peak.get();
            let mut peak = 0.0_f32;
            let mut sum = 0.0;
            let mut reference_dropped = 0;
            for frame in data.chunks_exact_mut(channels) {
                let values = if fault {
                    [0.0; 2]
                } else if speaker_test.active() {
                    let value = speaker_test.next_sample(c.volume.get(), cap);
                    [value; 2]
                } else {
                    synth.next_frame(gain, cap)
                };
                let reference_channels = if channels == 1 {
                    let mono = (values[0] + values[1]) * 0.5;
                    [mono; 2]
                } else {
                    values
                };
                if reference
                    .push(crate::auto_volume::ReferenceFrame {
                        channels: reference_channels,
                    })
                    .is_err()
                {
                    reference_dropped += 1;
                }
                for (ch, v) in frame.iter_mut().enumerate() {
                    let sample = if channels == 1 {
                        (values[0] + values[1]) * 0.5
                    } else {
                        values[ch % 2]
                    };
                    *v = T::from_sample(sample);
                    peak = peak.max(sample.abs());
                    sum += sample * sample;
                }
            }
            c.reference_dropped
                .fetch_add(reference_dropped, Ordering::Relaxed);
            c.output_frames
                .fetch_add((data.len() / channels) as u64, Ordering::Relaxed);
            c.test_active
                .store(speaker_test.active(), Ordering::Release);
            c.output_peak.set(peak);
            c.output_rms.set((sum / data.len().max(1) as f32).sqrt());
            let elapsed = began.elapsed().as_micros() as u64;
            c.callback_max_us.fetch_max(elapsed, Ordering::Relaxed);
            if elapsed as f64 > data.len() as f64 / channels as f64 / rate * 1e6 {
                c.callback_late.fetch_add(1, Ordering::Relaxed);
            }
        },
        move |e| record_stream_error(&err, &e, false),
        Some(Duration::from_secs(3)),
    )
    .map_err(|e| e.to_string())
}
struct WorkerConfig {
    settings: Settings,
    synthetic: bool,
    output_enabled: bool,
    rate: u32,
    output_rate: u32,
}
fn analyze_worker(
    mut samples: Consumer<f32>,
    mut references: Consumer<crate::auto_volume::ReferenceFrame>,
    mut params: Producer<Parameters>,
    c: Arc<Controls>,
    t: Arc<Mutex<Telemetry>>,
    config: WorkerConfig,
) {
    let WorkerConfig {
        settings: s,
        synthetic,
        output_enabled,
        rate,
        output_rate,
    } = config;
    let mut analyzer = Analyzer::new(rate as f32, s.fft_size);
    let mut sim = FanSimulator::new(s.scenario, rate as f32, s.seed);
    let began = Instant::now();
    let mut produced = 0_u64;
    let mut last_input = Instant::now();
    let mut calibration = crate::calibration::CalibrationSchedule::default();
    let mut root = 146.832;
    let mut held = 0.0;
    let mut policy = Parameters::default();
    let mut last_publish = Instant::now();
    let mut previous_rms = 0.0_f32;
    let mut growth_frames = 0;
    let mut interference = crate::duck::InterferenceGate::default();
    let mut ai = crate::policy::AiStatus::default();
    let mut inference = if output_enabled {
        match crate::policy::PolicyWorker::start() {
            Ok(worker) => Some(worker),
            Err(error) => {
                ai.actual_backend = "AI unavailable; neutral DSP".into();
                ai.fallback_reason = Some(error);
                None
            }
        }
    } else {
        ai.actual_backend = "Analysis only".into();
        None
    };
    let mut last_request = Instant::now() - Duration::from_secs(2);
    let mut calibrated = Features::default();
    let mut volume = crate::auto_volume::AdaptiveVolume::default();
    let mut reference_meter =
        crate::auto_volume::ReferenceMeter::new(output_rate as f32, s.fft_size);
    let mut reference_powers = [0.0; crate::auto_volume::BANDS];
    let mut reference_updated = Instant::now();
    let mut reference_seen = false;
    let mut previous_reference_drops = 0;
    let mut reference_drop_at: Option<Instant> = None;
    while !c.stop.load(Ordering::Acquire) {
        if c.fault.load(Ordering::Acquire) != 0 {
            break;
        }
        let now = Instant::now();
        if let Some(worker) = &mut inference {
            for _ in 0..4 {
                match worker.results.pop() {
                    Ok(packet) => {
                        policy = packet.parameters;
                        ai = packet.status;
                    }
                    Err(_) => break,
                }
            }
        }
        let reference_available = references.slots().min(16384);
        for _ in 0..reference_available {
            if let Ok(frame) = references.pop() {
                reference_meter.push(frame);
            }
        }
        if reference_available > 0 {
            if let Some(powers) = reference_meter.powers() {
                reference_powers = powers;
                reference_seen = true;
            }
            reference_updated = Instant::now();
        }
        let elapsed = began.elapsed();
        let scheduled_quiet = calibration.update(
            elapsed,
            c.calibrate.swap(false, Ordering::AcqRel),
            c.periodic_calibration.load(Ordering::Relaxed),
        );
        let quiet = !output_enabled || scheduled_quiet;
        c.quiet.store(quiet, Ordering::Release);
        let available = if synthetic {
            ((began.elapsed().as_secs_f64() * rate as f64) as u64)
                .saturating_sub(produced)
                .min(8192) as usize
        } else {
            samples.slots().min(8192)
        };
        if available == 0 {
            if !synthetic && last_input.elapsed() > Duration::from_secs(2) {
                c.fault.store(4, Ordering::Release);
                break;
            }
            thread::sleep(Duration::from_millis(3));
            continue;
        }
        last_input = now;
        for _ in 0..available {
            let value = if synthetic {
                produced += 1;
                sim.next_sample()
            } else {
                match samples.pop() {
                    Ok(v) => v,
                    Err(_) => break,
                }
            };
            if let Some(mut features) = analyzer.push(value) {
                let trusted = synthetic
                    || quiet
                        && calibration.settled(began.elapsed())
                        && c.output_rms.get() < 0.00001;
                if trusted && features.confidence > 0.45 && features.fan_hz > 0.0 {
                    held = features.fan_hz;
                    let mut candidate = held;
                    while candidate > 220.0 {
                        candidate *= 0.5;
                    }
                    while candidate < 110.0 {
                        candidate *= 2.0;
                    }
                    // Preserve fan detuning; change root only beyond 80 cents.
                    if (12.0 * (candidate / root).log2()).abs() > 0.8 {
                        root = candidate;
                    }
                }
                features.fan_hz = held;
                if !trusted {
                    features.confidence *= 0.25;
                }
                let duck_gain = interference.update(
                    &features,
                    began.elapsed().as_secs_f64(),
                    trusted && !synthetic,
                    c.duck.load(Ordering::Relaxed) && !synthetic,
                );
                c.attenuation.set(duck_gain);
                // Rising microphone energy during playback may be acoustic feedback or external sound.
                // Conservative emergency stop, with no claim to distinguish these sources.
                if !quiet
                    && !synthetic
                    && features.rms > previous_rms * 1.12
                    && features.dbfs > -12.0
                {
                    growth_frames += 1;
                } else {
                    growth_frames = 0;
                }
                previous_rms = features.rms;
                if growth_frames > 12 {
                    c.fault.store(5, Ordering::Release);
                }
                let mode = match c.mode.load(Ordering::Relaxed) {
                    1 => Mode::Nature,
                    2 => Mode::Generative,
                    _ => Mode::Harmonic,
                };
                let drops = c.reference_dropped.load(Ordering::Relaxed);
                if drops != previous_reference_drops {
                    reference_drop_at = Some(Instant::now());
                }
                let volume_status = volume.update(crate::auto_volume::Observation {
                    microphone: analyzer.band_powers(),
                    playback: reference_powers,
                    quiet,
                    trusted_quiet: trusted && quiet,
                    independent: synthetic,
                    reference_valid: reference_seen
                        && reference_updated.elapsed() < Duration::from_millis(250)
                        && reference_drop_at.is_none_or(|at| at.elapsed() > Duration::from_secs(2)),
                    mode,
                    seconds: s.fft_size as f32 / 4.0 / rate as f32,
                });
                previous_reference_drops = drops;
                c.adaptive_gain
                    .set(crate::auto_volume::gain_from_db(volume_status.gain_db));
                policy.mode = mode;
                policy.root_hz = root;
                policy.adaptation = c.adaptation.get();
                if trusted {
                    calibrated = features.clone();
                }
                if !trusted
                    && volume_status.confidence >= 0.25
                    && volume_status.state == "Following fan level"
                {
                    calibrated.dbfs = volume_status.fan_dbfs;
                    calibrated.rms = 10.0_f32.powf(volume_status.fan_dbfs / 20.0);
                }
                if last_request.elapsed() >= Duration::from_secs(1) {
                    if let Some(worker) = &mut inference {
                        let input = crate::policy::encode(
                            &calibrated,
                            policy,
                            began.elapsed().as_secs_f64(),
                        );
                        let _ = worker.requests.push(crate::policy::Job {
                            input,
                            parameters: policy,
                        });
                    }
                    last_request = Instant::now();
                }
                // During output retain calibrated parameters, including timbre, to avoid self-tracking.
                if trusted || last_publish.elapsed() > Duration::from_millis(100) {
                    let _ = params.push(policy);
                }
                if last_publish.elapsed() > Duration::from_millis(100) {
                    let mut snapshot = t.lock().unwrap_or_else(|e| e.into_inner());
                    snapshot.features = features;
                    snapshot.root_hz = root;
                    snapshot.backend = ai.actual_backend.clone();
                    snapshot.ai = ai.clone();
                    snapshot.auto_volume = volume_status;
                    snapshot.quiet = quiet;
                    snapshot.frozen = !trusted;
                    snapshot.status = if quiet {
                        "Quiet measurement; output fading/silent"
                    } else if synthetic {
                        "Synthetic analysis; no microphone"
                    } else {
                        "Microphone active; fan level adapts, pitch held between calibrations"
                    }
                    .into();
                    last_publish = Instant::now();
                }
            }
        }
    }
    c.quiet.store(true, Ordering::Release);
}
pub fn device_report() -> serde_json::Value {
    let host = cpal::default_host();
    let mut list = vec![];
    if let Ok(devices) = host.devices() {
        for d in devices {
            let input = d
                .supported_input_configs()
                .map(|i| i.map(|r| format!("{r:?}")).collect::<Vec<_>>())
                .unwrap_or_default();
            let output = d
                .supported_output_configs()
                .map(|i| i.map(|r| format!("{r:?}")).collect::<Vec<_>>())
                .unwrap_or_default();
            list.push(serde_json::json!({"name":d.to_string(),"id":d.id().map(|i|i.to_string()).ok(),"default_input":d.default_input_config().ok().map(|c|format!("{c:?}")),"default_output":d.default_output_config().ok().map(|c|format!("{c:?}")),"input_configs":input,"output_configs":output}));
        }
    }
    serde_json::json!({"host":format!("{:?}",host.id()),"default_input_name":host.default_input_device().map(|d|d.to_string()),"default_output_name":host.default_output_device().map(|d|d.to_string()),"raw_capture":"not negotiated: normal shared WASAPI path; APO/AGC status unknown","devices":list})
}

fn record_stream_error(c: &Controls, error: &cpal::Error, input: bool) {
    let code = error.kind() as u32 + 1;
    if input {
        c.input_error_kind.store(code, Ordering::Relaxed);
    } else {
        c.output_error_kind.store(code, Ordering::Relaxed);
    }
    match error.kind() {
        cpal::ErrorKind::Xrun => {
            c.xruns.fetch_add(1, Ordering::Relaxed);
        }
        cpal::ErrorKind::RealtimeDenied => {
            c.realtime_warnings.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            c.fault.store(if input { 1 } else { 2 }, Ordering::Release);
        }
    }
}
pub fn error_kind_name(code: u32) -> String {
    if code == 0 {
        return "None".into();
    }
    [
        cpal::ErrorKind::DeviceBusy,
        cpal::ErrorKind::DeviceChanged,
        cpal::ErrorKind::DeviceNotAvailable,
        cpal::ErrorKind::HostUnavailable,
        cpal::ErrorKind::InvalidInput,
        cpal::ErrorKind::PermissionDenied,
        cpal::ErrorKind::RealtimeDenied,
        cpal::ErrorKind::ResourceExhausted,
        cpal::ErrorKind::StreamInvalidated,
        cpal::ErrorKind::UnsupportedConfig,
        cpal::ErrorKind::UnsupportedOperation,
        cpal::ErrorKind::Xrun,
        cpal::ErrorKind::BackendError,
        cpal::ErrorKind::Other,
    ]
    .into_iter()
    .find(|k| *k as u32 + 1 == code)
    .map(|k| format!("{k:?}"))
    .unwrap_or_else(|| format!("Unknown({code})"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recoverable_stream_events_are_counted_and_device_loss_stops() {
        let c = Controls::new(&Settings::default());
        record_stream_error(&c, &cpal::ErrorKind::Xrun.into(), true);
        record_stream_error(&c, &cpal::ErrorKind::RealtimeDenied.into(), true);
        assert_eq!(c.fault.load(Ordering::Relaxed), 0);
        assert_eq!(c.xruns.load(Ordering::Relaxed), 1);
        assert_eq!(c.realtime_warnings.load(Ordering::Relaxed), 1);
        record_stream_error(&c, &cpal::ErrorKind::DeviceNotAvailable.into(), false);
        assert_eq!(c.fault.load(Ordering::Relaxed), 2);
        assert_eq!(
            error_kind_name(c.output_error_kind.load(Ordering::Relaxed)),
            "DeviceNotAvailable"
        );
    }
}
