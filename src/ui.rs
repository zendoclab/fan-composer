use fan_composer::{
    audio::{self, AudioSession, DeviceChoice, Telemetry},
    settings::Settings,
    simulator::Scenario,
    synth::Mode,
};
use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Subscription, Task, Theme, mouse,
    widget::{
        self, button, canvas, checkbox, column, container, pick_list, row, scrollable, slider, text,
    },
};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub enum Message {
    Start,
    SpeakerTest,
    Mixer(
        String,
        Result<fan_composer::windows_audio::MixerStatus, String>,
    ),
    Stop,
    Calibrate,
    Tick,
    Mode(Mode),
    Volume(f32),
    Cap(f32),
    Speed(f32),
    Duck(bool),
    PeriodicCalibration(bool),
    Advanced(bool),
    Synthetic(bool),
    Scenario(Scenario),
    Input(DeviceChoice),
    Output(DeviceChoice),
    Fft(usize),
    Feedback(i32),
    Refresh,
    Screenshot(iced::window::Screenshot),
}
pub struct App {
    settings: Settings,
    inputs: Vec<DeviceChoice>,
    outputs: Vec<DeviceChoice>,
    input: DeviceChoice,
    output: DeviceChoice,
    session: Option<AudioSession>,
    snapshot: Telemetry,
    synthetic: bool,
    status: String,
    smoke: Option<Instant>,
    smoke_captured: bool,
    test_started: Option<Instant>,
    last_mixer: Instant,
    mixer_pending: bool,
    mixer_summary: String,
    advanced: bool,
    save_pending: Option<Instant>,
}
impl App {
    pub fn new() -> Self {
        let mut settings = Settings::load();
        let args: Vec<String> = std::env::args().collect();
        if let Some(index) = args.iter().position(|s| s == "--speaker")
            && let Some(id) = args.get(index + 1)
        {
            settings.output = Some(id.clone());
        }
        let inputs = audio::choices(true);
        let outputs = audio::choices(false);
        let input = inputs
            .iter()
            .find(|d| Some(&d.id) == settings.input.as_ref())
            .cloned()
            .unwrap_or_else(|| inputs[0].clone());
        let output = outputs
            .iter()
            .find(|d| Some(&d.id) == settings.output.as_ref())
            .cloned()
            .unwrap_or_else(|| outputs[0].clone());
        let smoke = std::env::args()
            .any(|s| s == "--ui-smoke")
            .then(Instant::now);
        Self {
            settings,
            inputs,
            outputs,
            input,
            output,
            session: None,
            snapshot: Telemetry::default(),
            synthetic: false,
            status: "Stopped. Microphone closed. Press Start to begin.".into(),
            smoke,
            smoke_captured: false,
            test_started: None,
            last_mixer: Instant::now() - Duration::from_secs(3),
            mixer_pending: false,
            mixer_summary: "Reading Windows output volume…".into(),
            advanced: false,
            save_pending: None,
        }
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        if matches!(
            message,
            Message::Mode(_)
                | Message::Volume(_)
                | Message::Cap(_)
                | Message::Speed(_)
                | Message::Duck(_)
                | Message::PeriodicCalibration(_)
                | Message::Scenario(_)
                | Message::Input(_)
                | Message::Output(_)
                | Message::Fft(_)
                | Message::Start
        ) {
            self.save_pending = Some(Instant::now());
        }
        match message {
            Message::Mixer(id, result) => {
                self.mixer_pending = false;
                if id == self.output.id {
                    self.mixer_summary = match result {
                        Ok(m) => format!(
                            "Windows {:.0}% ({:.1} dB){} | App mixer: {}",
                            m.master_volume * 100.0,
                            m.master_db,
                            if m.master_muted { " MUTED" } else { "" },
                            match (m.session_volume, m.session_muted) {
                                (Some(v), Some(muted)) => format!(
                                    "{:.0}%{}",
                                    v * 100.0,
                                    if muted { " MUTED" } else { "" }
                                ),
                                _ => "no active session".into(),
                            }
                        ),
                        Err(e) => format!("Windows mixer unavailable: {e}"),
                    };
                }
            }
            Message::SpeakerTest => {
                if self.session.is_none() {
                    match AudioSession::speaker_test(&self.settings) {
                        Ok(s) => {
                            self.session = Some(s);
                            self.test_started = Some(Instant::now());
                            self.status = "500 Hz test for 2 s. Microphone OFF.".into();
                        }
                        Err(e) => self.status = format!("Speaker test failed: {e}"),
                    }
                }
            }
            Message::Start => {
                if self.session.is_none() {
                    match AudioSession::start(&self.settings, self.synthetic, true) {
                        Ok(s) => {
                            self.session = Some(s);
                            self.status = "Started. Quiet calibration before sound.".into();
                        }
                        Err(e) => self.status = format!("Start failed: {e}"),
                    }
                }
            }
            Message::Stop => {
                self.session.take();
                self.test_started = None;
                self.status = "Stopped. Microphone closed.".into();
            }
            Message::Calibrate => {
                if let Some(s) = &self.session {
                    s.controls.calibrate.store(true, Ordering::Release);
                }
            }
            Message::Tick => {
                if self
                    .save_pending
                    .is_some_and(|at| at.elapsed() > Duration::from_millis(900))
                {
                    self.save_pending = None;
                    if let Err(error) = self.settings.save() {
                        self.status = format!("Could not save settings: {error}");
                    }
                }
                if let Some(s) = &self.session {
                    self.snapshot = s.snapshot();
                    let fault = s.controls.fault.load(Ordering::Acquire);
                    if fault != 0 {
                        self.session.take();
                        self.test_started = None;
                        self.status = format!(
                            "Safety stop (code {fault}). Check audio devices; restart explicitly."
                        );
                    }
                }
                if self
                    .test_started
                    .is_some_and(|t| t.elapsed() > Duration::from_millis(2300))
                {
                    self.session.take();
                    self.test_started = None;
                    self.status = "Speaker test finished. Press Start for soundscapes.".into();
                }
                if !self.mixer_pending && self.last_mixer.elapsed() > Duration::from_secs(2) {
                    self.last_mixer = Instant::now();
                    self.mixer_pending = true;
                    let id = self.output.id.clone();
                    let result_id = id.clone();
                    return Task::perform(
                        async move {
                            fan_composer::windows_audio::output_mixer(Some(id), std::process::id())
                        },
                        move |result| Message::Mixer(result_id.clone(), result),
                    );
                }
                if self
                    .smoke
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(2))
                    && !self.smoke_captured
                {
                    self.smoke_captured = true;
                    return iced::window::oldest().then(|id| {
                        if let Some(id) = id {
                            iced::window::screenshot(id).map(Message::Screenshot)
                        } else {
                            iced::exit()
                        }
                    });
                }
                if self
                    .smoke
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(8))
                {
                    return iced::exit();
                }
            }
            Message::Mode(v) => self.settings.mode = v,
            Message::Volume(v) => {
                self.settings.volume = (v / 100.0).clamp(0.0, 1.0)
                    * fan_composer::level::db_to_gain(fan_composer::level::MAX_LEVEL_DB)
            }
            Message::Cap(v) => self.settings.max_peak = v,
            Message::Speed(v) => self.settings.adaptation = v,
            Message::Duck(v) => self.settings.duck = v,
            Message::PeriodicCalibration(v) => self.settings.periodic_calibration = v,
            Message::Advanced(v) => self.advanced = v,
            Message::Synthetic(v) => {
                if self.session.is_none() {
                    self.synthetic = v;
                }
            }
            Message::Scenario(v) => {
                if self.session.is_none() {
                    self.settings.scenario = v;
                }
            }
            Message::Fft(v) => {
                if self.session.is_none() {
                    self.settings.fft_size = v;
                }
            }
            Message::Input(v) => {
                if self.session.is_none() {
                    self.settings.input = Some(v.id.clone());
                    self.input = v;
                }
            }
            Message::Output(v) => {
                if self.session.is_none() {
                    self.settings.output = Some(v.id.clone());
                    self.output = v;
                }
            }
            Message::Refresh => {
                if self.session.is_none() {
                    self.inputs = audio::choices(true);
                    self.outputs = audio::choices(false);
                    self.status = "Device list refreshed.".into();
                }
            }
            Message::Feedback(v) => match self.settings.feedback(v, &self.snapshot.features) {
                Ok(()) => self.status = "Feedback saved locally (features only).".into(),
                Err(e) => self.status = e,
            },
            Message::Screenshot(shot) => {
                let mut ppm =
                    format!("P6\n{} {}\n255\n", shot.size.width, shot.size.height).into_bytes();
                for rgba in shot.rgba.chunks_exact(4) {
                    ppm.extend_from_slice(&rgba[..3]);
                }
                let _ = std::fs::write("reports/ui-smoke.ppm", ppm);
                return iced::exit();
            }
        }
        if let Some(s) = &self.session {
            s.controls.apply(&self.settings);
        }
        Task::none()
    }
    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick)
    }
    pub fn view(&self) -> Element<'_, Message> {
        let running = self.session.is_some();
        let testing = self.test_started.is_some();
        let volume = &self.snapshot.auto_volume;
        let maximum = self.settings.volume
            / fan_composer::level::db_to_gain(fan_composer::level::MAX_LEVEL_DB)
            * 100.0;
        let backend = if !running || testing {
            "NPU first · CPU fallback"
        } else if self.snapshot.backend == "NPU" {
            "NPU active"
        } else if self.snapshot.backend.starts_with("CPU") {
            "CPU fallback active"
        } else {
            "Starting music policy…"
        };
        let mut modes = row![].spacing(8);
        for mode in Mode::ALL {
            let selected = self.settings.mode == mode;
            modes = modes.push(
                button(text(mode.to_string()).center().width(Length::Fill))
                    .on_press(Message::Mode(mode))
                    .style(if selected {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .width(Length::Fill),
            );
        }
        let mode_hint = match self.settings.mode {
            Mode::Harmonic => "A soft, steady harmony around the fan.",
            Mode::Nature => "Gentle wind and wave-like noise textures.",
            Mode::Generative => "Slow melodies with a continuous ambient background.",
        };
        let live_hint = if !running {
            "Press Start. The first fan measurement takes 3.5 seconds."
        } else if testing {
            "Speaker test. Microphone is off."
        } else if self.snapshot.quiet {
            "Measuring the fan. Sound starts when the measurement finishes."
        } else if volume.state == "Following fan level" {
            "Sound follows the fan and stays below your maximum."
        } else {
            "Sound continues. Level adjusts cautiously while the fan estimate settles."
        };
        let level_text = if running && !testing {
            format!(
                "Sound level: {:.0}% of your maximum",
                fan_composer::auto_volume::gain_from_db(volume.gain_db) * 100.0
            )
        } else {
            "Automatic volume is always on.".into()
        };
        let actions = row![
            button(if running { "Stop" } else { "Start" })
                .on_press(if running {
                    Message::Stop
                } else {
                    Message::Start
                })
                .width(Length::Fill),
            button("Measure fan again")
                .on_press_maybe((running && !testing).then_some(Message::Calibrate))
        ]
        .spacing(10);
        let header = row![
            text("Fan Composer").size(27),
            widget::Space::new().width(Length::Fill),
            text(backend).size(13)
        ]
        .spacing(10);
        let speaker = row![
            text("Speaker").width(65),
            pick_list(self.outputs.as_slice(), Some(&self.output), Message::Output)
                .width(Length::Fill)
        ]
        .spacing(8);
        let volume_controls = column![
            row![
                text(format!("Maximum volume  {:.0}%", maximum.clamp(0.0, 100.0))),
                widget::Space::new().width(Length::Fill),
                text(if self.settings.volume == 0.0 {
                    "Muted"
                } else {
                    "Auto"
                })
                .size(13)
            ],
            slider(0.0..=100.0, maximum.clamp(0.0, 100.0), Message::Volume).step(1.0_f32),
            text("Fan gets quieter → sound gets quieter. Fan gets louder → sound rises gently.")
                .size(12),
            text(level_text).size(13)
        ]
        .spacing(9);
        let main = column![
            text("Turning Machine Noise into Music").size(14),
            speaker,
            modes,
            text(mode_hint).size(13),
            volume_controls,
            text(live_hint).size(13),
            text(if running && !self.synthetic && !testing {
                "Microphone active · processed locally"
            } else {
                "Microphone off"
            })
            .size(12)
            .color(Color::from_rgb(0.35, 0.8, 0.67)),
            canvas(Spectrum(&self.snapshot.features.spectrum))
                .width(Length::Fill)
                .height(90),
            row![
                text("Live microphone spectrum").size(12),
                widget::Space::new().width(Length::Fill),
                button(if self.advanced {
                    "Hide advanced"
                } else {
                    "Advanced"
                })
                .on_press(Message::Advanced(!self.advanced))
            ]
        ]
        .spacing(13);
        let mut body = column![main].spacing(18);
        if self.advanced {
            let f = &self.snapshot.features;
            let mut diagnostics = String::from("No audio streams active.");
            if let Some(s) = &self.session {
                let c = &s.controls;
                diagnostics = format!(
                    "{} / {} Hz | capture/playback frames {} / {}\nInput/reference drops {} / {} | callback max {} us | over-budget {} | xruns {}\nOutput RMS {:.1} / peak {:.1} dBFS\nPolicy: {} | NPU {} / CPU {} / rules {} decisions | last {:.2} ms\n{}",
                    self.snapshot.input_rate,
                    self.snapshot.output_rate,
                    c.input_frames.load(Ordering::Relaxed),
                    c.output_frames.load(Ordering::Relaxed),
                    c.input_dropped.load(Ordering::Relaxed),
                    c.reference_dropped.load(Ordering::Relaxed),
                    c.callback_max_us.load(Ordering::Relaxed),
                    c.callback_late.load(Ordering::Relaxed),
                    c.xruns.load(Ordering::Relaxed),
                    fan_composer::level::gain_to_db(c.output_rms.get()),
                    fan_composer::level::gain_to_db(c.output_peak.get()),
                    self.snapshot.backend,
                    self.snapshot.ai.npu_decisions,
                    self.snapshot.ai.cpu_decisions,
                    self.snapshot.ai.rule_decisions,
                    self.snapshot.ai.last_inference_ms,
                    self.snapshot.ai.fallback_reason.as_deref().unwrap_or("")
                );
            }
            let advanced = column![
                text("Audio & diagnostics").size(18),
                row![text("Microphone").width(90),
                    pick_list(self.inputs.as_slice(),Some(&self.input),Message::Input).width(Length::Fill)].spacing(8),
                row![checkbox(self.synthetic).label("Synthetic fan (microphone off)").on_toggle(Message::Synthetic),
                    pick_list(Scenario::ALL,Some(self.settings.scenario),Message::Scenario)].spacing(8),
                row![text("FFT"),pick_list([2048_usize,4096],Some(self.settings.fft_size),Message::Fft),
                    button("Refresh devices").on_press(Message::Refresh),
                    button("Test speaker (2 s)").on_press_maybe((!running).then_some(Message::SpeakerTest))].spacing(8),
                row![text(format!("Peak cap {:.1}%",self.settings.max_peak*100.0)).width(125),
                    slider(0.005..=0.15,self.settings.max_peak,Message::Cap).step(0.005_f32)].spacing(8),
                row![text("Adaptation speed").width(125),
                    slider(0.05..=1.0,self.settings.adaptation,Message::Speed).step(0.05_f32)].spacing(8),
                checkbox(self.settings.duck).label("Reduce sound during sudden interference (experimental)").on_toggle(Message::Duck),
                checkbox(self.settings.periodic_calibration).label("Repeat quiet measurement every 30 s (interrupts sound)").on_toggle(Message::PeriodicCalibration),
                text(format!("Raw input {:.1} dBFS | peak {:.1} Hz | held fan pitch {:.1} Hz\nFan/background band estimate {:.1} dBFS | calibration {:.1} dBFS\nAuto gain {:.1} dB of maximum | target {:.1} dB | usable coverage {:.0}%\n{}",
                    f.dbfs,f.peak_hz,f.fan_hz,volume.fan_dbfs,volume.reference_dbfs,
                    volume.gain_db,volume.target_db,volume.confidence*100.0,volume.state)).size(12),
                text(&self.mixer_summary).size(12),
                text(diagnostics).size(12),
                text("dBFS is not dB SPL. The fan/background estimate and speaker leakage guard are not acoustic echo cancellation. Fan pitch is held between quiet measurements.").size(12),
                text(concat!("Version ",env!("CARGO_PKG_VERSION"))).size(12)
            ].spacing(12);
            body = body.push(
                container(advanced)
                    .padding(14)
                    .style(container::rounded_box),
            );
        }
        body = body.push(
            row![
                text("Comfort").size(13),
                button("Worse").on_press(Message::Feedback(-1)),
                button("Same").on_press(Message::Feedback(0)),
                button("Better").on_press(Message::Feedback(1))
            ]
            .spacing(8),
        );
        body = body.push(text(&self.status).size(12));
        body = body.push(
            text("Settings save automatically. No microphone recording or cloud processing.")
                .size(11),
        );
        column![
            container(column![header, actions].spacing(12))
                .padding(18)
                .width(Length::Fill),
            scrollable(container(body).padding(22).max_width(680)).height(Length::Fill)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
struct Spectrum<'a>(&'a [f32]);
impl canvas::Program<Message> for Spectrum<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(
            Point::ORIGIN,
            bounds.size(),
            Color::from_rgb(0.06, 0.08, 0.11),
        );
        for db in [-90.0, -60.0, -30.0] {
            let y = bounds.height * (1.0 - (db + 110.0) / 110.0);
            frame.fill_rectangle(
                Point::new(0.0, y),
                Size::new(bounds.width, 1.0),
                Color::from_rgb(0.16, 0.2, 0.24),
            );
        }
        let w = bounds.width / self.0.len().max(1) as f32;
        for (i, db) in self.0.iter().enumerate() {
            let h = ((db + 110.0) / 110.0).clamp(0.0, 1.0) * bounds.height;
            frame.fill_rectangle(
                Point::new(i as f32 * w, bounds.height - h),
                Size::new((w - 1.0).max(1.0), h),
                Color::from_rgb(0.3, 0.72, 0.65),
            );
        }
        vec![frame.into_geometry()]
    }
}
pub fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(concat!("Fan Composer ", env!("CARGO_PKG_VERSION")))
        .subscription(App::subscription)
        .theme(Theme::TokyoNight)
        .window(iced::window::Settings {
            size: Size::new(660.0, 800.0),
            min_size: Some(Size::new(540.0, 560.0)),
            ..Default::default()
        })
        .centered()
        .run()
}
