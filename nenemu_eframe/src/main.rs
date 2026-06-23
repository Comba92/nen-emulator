// this removes the windows console
// #![windows_subsystem = "windows"]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use nenemu_core::{NesPalette, emu::NesEmulator, joypad, rom};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    thread, time,
};

const TEX_OPTS: egui::TextureOptions = egui::TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Nearest,
    wrap_mode: egui::TextureWrapMode::ClampToEdge,
    mipmap_mode: None,
};

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
enum EmulatorAction {}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
enum PlayerEvent {
    Joypad(joypad::InputBtn),
    Action(EmulatorAction),
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct KeyMap {
    btns_strings: HashMap<joypad::InputBtn, String>,
    keys: HashMap<egui::Key, joypad::InputBtn>,
    pads: HashMap<gilrs::Button, joypad::InputBtn>,
    rebind_key: Option<(egui::Key, joypad::InputBtn)>,
}

impl Default for KeyMap {
    fn default() -> Self {
        use egui::Key;
        use joypad::InputBtn as Btn;

        let keys = HashMap::from([
            (Key::ArrowUp, Btn::Up),
            (Key::ArrowDown, Btn::Down),
            (Key::ArrowLeft, Btn::Left),
            (Key::ArrowRight, Btn::Right),
            (Key::S, Btn::A),
            (Key::A, Btn::B),
            (Key::W, Btn::Start),
            (Key::E, Btn::Select),
        ]);

        let pads = {
            use gilrs::Button;
            HashMap::from([
                (Button::DPadUp, Btn::Up),
                (Button::DPadDown, Btn::Down),
                (Button::DPadLeft, Btn::Left),
                (Button::DPadRight, Btn::Right),
                (Button::South, Btn::A),
                (Button::West, Btn::B),
                (Button::Start, Btn::Start),
                (Button::Select, Btn::Select),
            ])
        };

        let btns_strings = HashMap::from_iter(
            [
                (Btn::Up, "UP"),
                (Btn::Down, "DOWN"),
                (Btn::Left, "LEFT"),
                (Btn::Right, "RIGHT"),
                (Btn::A, "A"),
                (Btn::B, "B"),
                (Btn::Start, "Start"),
                (Btn::Select, "Select"),
            ]
            .iter()
            .map(|x| (x.0, x.1.to_string())),
        );

        Self {
            keys,
            pads,
            btns_strings,
            rebind_key: None,
        }
    }
}

struct GamepadHandler {
    pub api: Option<gilrs::Gilrs>,
    // for now, we only handle the first active gamepad
    pub active: Option<gilrs::GamepadId>,
}
impl GamepadHandler {
    pub fn poll(
        &mut self,
        current_input: nenemu_core::joypad::InputBtn,
        keymaps: &KeyMap,
    ) -> nenemu_core::joypad::InputBtn {
        let mut gamepad_input = current_input;

        while let Some(gilrs::Event { id, event, .. }) =
            self.api.as_mut().and_then(|api| api.next_event())
        {
            if event == gilrs::EventType::Connected {
                self.active = Some(id);
            }

            if let Some(active) = self.active {
                if active != id {
                    continue;
                }

                match event {
                    gilrs::EventType::Disconnected => {
                        if self.active.filter(|x| *x == id).is_some() {
                            self.active = None;
                        }
                    }

                    gilrs::EventType::ButtonReleased(btn, _) => {
                        if let Some(emu_btn) = keymaps.pads.get(&btn) {
                            gamepad_input.remove(*emu_btn);
                        }
                    }

                    gilrs::EventType::ButtonPressed(btn, _) => {
                        if let Some(emu_btn) = keymaps.pads.get(&btn) {
                            gamepad_input.insert(*emu_btn);
                        }
                    }

                    gilrs::EventType::AxisChanged(axis, amt, _) => match axis {
                        gilrs::Axis::LeftStickX => {
                            if amt >= 0.1 {
                                gamepad_input.insert(joypad::InputBtn::Right);
                            } else if amt <= -0.1 {
                                gamepad_input.insert(joypad::InputBtn::Left);
                            } else {
                                gamepad_input.remove(joypad::InputBtn::Right);
                                gamepad_input.remove(joypad::InputBtn::Left);
                            }
                        }
                        gilrs::Axis::LeftStickY => {
                            if amt >= 0.1 {
                                gamepad_input.insert(joypad::InputBtn::Up);
                            } else if amt <= -0.1 {
                                gamepad_input.insert(joypad::InputBtn::Down);
                            } else {
                                gamepad_input.remove(joypad::InputBtn::Up);
                                gamepad_input.remove(joypad::InputBtn::Down);
                            }
                        }
                        _ => {}
                    },

                    _ => {}
                }
            }
        }

        gamepad_input
    }
}

impl Default for GamepadHandler {
    fn default() -> Self {
        let api = gilrs::Gilrs::new()
            .inspect_err(|err| eprintln!("{err}"))
            .ok();

        let active = api
            .as_ref()
            .and_then(|api| api.gamepads().next().map(|x| x.0));
        Self { api, active }
    }
}

fn cpal_callback(emu: &Arc<Mutex<NesEmulator>>, volume: &Arc<Mutex<f32>>, audio_out: &mut [f32]) {
    let mut emu_lock = emu.lock().unwrap();

    while emu_lock.audio_queued() < audio_out.len() {
        emu_lock.step();
    }

    let volume = *volume.lock().unwrap();

    let samples = emu_lock.get_audio_f32_iter(audio_out.len() / 2);
    for (i, sample) in samples.enumerate() {
        audio_out[2 * i] = sample * volume;
        audio_out[2 * i + 1] = sample * volume;
    }

    // let (right, left) = emu_lock.get_audio_f32(audio_out.len() / 2);
    // for i in 0..right.len() {
    //     audio_out[2 * i] = right[i] * volume;
    //     audio_out[2 * i + 1] = right[i] * volume;
    // }

    // if let Some(left) = left {
    //     let audio_out = &mut audio_out[2 * right.len()..];
    //     for i in 0..left.len() {
    //         audio_out[2 * i] = left[i] * volume;
    //         audio_out[2 * i + 1] = left[i] * volume;
    //     }
    // }
}

struct AudioStreamData {
    cb: cpal::Stream,
    device: cpal::Device,
    cfg: cpal::StreamConfig
}

struct AudioHandler {
    host: cpal::Host,
    stream: Option<AudioStreamData>,
    sample_rate: u32,
    buf_size: u32,
    devices: Vec<cpal::Device>,
    enabled: bool,
    volume: Arc<Mutex<f32>>,
}

impl AudioHandler {
    pub fn new(
        sample_rate: u32,
        buf_size: u32,
        emu: Arc<Mutex<NesEmulator>>,
        enabled: bool,
    ) -> Self {
        let host = cpal::default_host();

        let devices = host
            .output_devices()
            .map(|devices| devices.collect())
            .unwrap_or_default();

        // take the default device for now
        match host.default_output_device() {
            Some(device) => {
                let mut good_cfgs = device
                    .supported_output_configs()
                    .unwrap()
                    .filter(|cfg| cfg.channels() == 2 && cfg.sample_format() == cpal::SampleFormat::F32)
                    .filter_map(|cfg| cfg.try_with_standard_sample_rate());

                let volume = Arc::new(Mutex::new(0.5));

                let stream = good_cfgs.next().and_then(|cfg| {
                    let volume_arc = Arc::clone(&volume);
                    device
                        .build_output_stream(
                            cfg.into(),
                            move |audio_out, _| cpal_callback(&emu, &volume_arc, audio_out),
                            |err| eprintln!("Cpal callback error: {err}"),
                            None,
                        )
                        .inspect_err(|e| eprintln!("Cpal creation error {e}"))
                        .ok()
                        .and_then(|cb| Some(AudioStreamData {
                            cb,
                            device,
                            cfg: cfg.into(),
                        }))
                });

                // let config = cpal::StreamConfig {
                //     channels: 2,
                //     sample_rate: sample_rate,
                //     buffer_size: cpal::BufferSize::Fixed(buf_size),
                // };

                // let stream = device
                //     .build_output_stream(
                //         config,
                //         move |audio_out, _| cpal_callback(&emu, &volume_arc, audio_out),
                //         |err| eprintln!("{err}"),
                //         None,
                //     )
                //     .inspect_err(|e| eprintln!("{e}"))
                //     .ok();

                let res = Self {
                    host,
                    stream,
                    sample_rate,
                    buf_size,
                    devices,
                    enabled,
                    volume,
                };

                res
            }

            None => Self {
                host,
                stream: None,
                sample_rate,
                buf_size,
                devices: Vec::new(),
                enabled: false,
                volume: Default::default(),
            },
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.stream.is_some() && self.enabled
    }

    pub fn set_enabled(&mut self, cond: bool, emu: Arc<Mutex<NesEmulator>>) {
        self.enabled = cond;
        self.clear(emu);

        match &self.stream {
            Some(stream) => {
                if self.enabled {
                    _ = stream.cb.play();
                } else {
                    _ = stream.cb.pause();
                }
            }
            None => {}
        }
    }

    fn set_ouput_device(&mut self, emu: Arc<Mutex<NesEmulator>>) {
        todo!()
    }

    fn set_sample_rate(&mut self, rate: nenemu_core::emu::SampleRate, emu: Arc<Mutex<NesEmulator>>) {
        if let Some(stream) = self.stream.take() {
            todo!()
        }
    }

    pub fn buffer_size(&self) -> usize {
        self.stream
            .as_ref()
            .map(|s| s.cb.buffer_size().unwrap_or_default())
            .unwrap_or_default() as usize
    }

    pub fn clear(&mut self, emu: Arc<Mutex<NesEmulator>>) {
        if let Some(mut stream) = self.stream.take() {
            let volume_arc = Arc::clone(&self.volume);
            let cb = stream.device.build_output_stream(
                stream.cfg,
                move |buf, _| cpal_callback(&emu, &volume_arc, buf),
                |err| eprintln!("Cpal callback error: {err}"),
                None
            );

            _ = stream.cb.pause();
            if let Ok(cb) = cb {
                stream.cb = cb;
                self.stream = Some(stream);
            } else {
                self.stream = None;
            }
        } else {
            // TODO: find another device
        }
    }

    pub fn resume(&self) {
        if !self.enabled {
            return;
        }

        match &self.stream {
            Some(stream) => _ = stream.cb.play(),
            _ => {}
        }
    }

    pub fn pause(&self) {
        if !self.enabled {
            return;
        }

        match &self.stream {
            Some(stream) => _ = stream.cb.pause(),
            _ => {}
        }
    }
}

// struct AudioThread {
//     emu: Arc<Mutex<NesEmulator>>,
// }
// impl sdl2::audio::AudioCallback for AudioThread {
//     type Channel = f32;

//     fn callback(&mut self, audio_out: &mut [Self::Channel]) {
//         let mut emu_lock = self.emu.lock().unwrap();

//         let (right, left) = emu_lock.get_audio_f32(audio_out.len());
//         let right_amt = right.len();
//         audio_out[..right_amt].copy_from_slice(right);

//         if let Some(left) = left {
//             audio_out[right_amt..].copy_from_slice(left);
//         }
//     }
// }

// struct SdlCtx {
//     sdl: sdl2::Sdl,
//     audio: sdl2::AudioSubsystem,
//     audiodev: sdl2::audio::AudioDevice<AudioThread>,
//     samplebuf_size: usize,
// }

// impl SdlCtx {
//     pub fn new(sample_rate: usize, emu: Arc<Mutex<NesEmulator>>) -> Self {
//         let sdl = sdl2::init().unwrap();
//         let audio = sdl.audio().unwrap();
//         let audiospec = sdl2::audio::AudioSpecDesired {
//             channels: Some(1),
//             freq: Some(sample_rate as i32),
//             samples: Some(1024),
//         };

//         let audiodev = audio
//             .open_playback(None, &audiospec, |_| AudioThread { emu })
//             .unwrap();
//         // audiodev.resume();

//         let samplebuf_size = audiodev.spec().samples as usize;
//         Self {
//             sdl,
//             audio,
//             audiodev,
//             samplebuf_size,
//         }
//     }
// }

fn sleep_until_fps(frame_start: time::Instant, frame_rate: time::Duration) {
    let frame_duration = frame_start.elapsed();
    if frame_duration < frame_rate {
        thread::sleep(frame_rate - frame_duration);
    }
}

fn buffered_read<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, GenericError> {
    use std::io::Read;

    let mut bytes = Vec::new();
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

// fn emulation_thread_proc(
//     emu: Arc<Mutex<NesEmulator>>,
//     video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
//     samples_needed: usize,
//     is_running: Arc<atomic::AtomicBool>,
// ) {
//     let frame_rate = time::Duration::from_secs_f32(1.0 / 288.0);
//     loop {
//         let frame_start = time::Instant::now();

//         if is_running.load(atomic::Ordering::Relaxed) {
//             let mut emu_lock = emu.lock().unwrap();
//             while emu_lock.audio_queued() < samples_needed {
//                 emu_lock
//                     .step_until_samples_or_frame_ready(samples_needed)
//                     .unwrap();

//                 if emu_lock.is_frame_ready() {
//                     let framebuf = egui::ColorImage::from_rgba_unmultiplied(
//                         [256, 240],
//                         emu_lock.get_video_rgba(),
//                     );
//                     video_chain.lock().unwrap().push(framebuf);
//                 }
//             }
//         }

//         sleep_until_fps(frame_start, frame_rate);
//     }
// }

// fn emulation_thread_no_audio_proc(
//     emu: Arc<Mutex<NesEmulator>>,
//     video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
//     is_running: Arc<atomic::AtomicBool>,
// ) {
//     let frame_rate = time::Duration::from_secs_f32(1.0 / 61.0);
//     loop {
//         let frame_start = time::Instant::now();

//         if is_running.load(atomic::Ordering::Relaxed) {
//             let mut emu_lock = emu.lock().unwrap();
//             emu_lock.step_until_frame_ready().unwrap();

//             let framebuf =
//                 egui::ColorImage::from_rgba_unmultiplied([256, 240], emu_lock.get_video_rgba());
//             video_chain.lock().unwrap().push(framebuf);
//         }

//         sleep_until_fps(frame_start, frame_rate);
//     }
// }

#[derive(Default, PartialEq, Clone, Copy, Debug)]
enum EmulationState {
    #[default]
    Stopped,
    Running,
    Paused,
}

// #[derive(Default, PartialEq, Clone, Copy)]
// enum RefreshRate {
//     Fps60 = 60,
//     #[default]
//     Fps120 = 120,
//     Fps144 = 144,
// }
// impl RefreshRate {
//     pub fn fps(&self) -> f32 {
//         1.0 / *self as usize as f32
//     }
// }

const APP_NAME: &'static str = "NenEmu";
type GenericError = Box<dyn std::error::Error>;

fn main() {
    let opts = eframe::NativeOptions {
        centered: true,
        viewport: egui::ViewportBuilder::default()
            .with_drag_and_drop(true)
            .with_inner_size((256.0 * 3.0, 240.0 * 3.0))
            .with_title(APP_NAME),
        vsync: true,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,

        ..Default::default()
    };

    eframe::run_native(APP_NAME, opts, Box::new(|c| Ok(AppCtx::new(c)))).unwrap();
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct AppCfg {
    keymaps: KeyMap,
    recent_roms: VecDeque<PathBuf>,
    palettes: VecDeque<NesPalette>,
    bios_path: Option<PathBuf>,
    keep_aspect_ratio: bool,
    fullscreen: bool,
    hide_cursor: bool,
    battery_save_enabled: bool,

    #[cfg(feature = "persistence")]
    restore_session: bool,

    nes_settings: nenemu_core::emu::NesSettings,

    disable_audio: bool,
    // refresh_rate: RefreshRate,
    volume: f32,
    sample_rate: nenemu_core::emu::SampleRate,
}

#[derive(Default)]
struct AppState {
    should_close: bool,
    exit_modal_open: bool,

    keybinds_open: bool,
    settings_open: bool,
    rom_info_open: bool,
    about_open: bool,
    message_open: Option<(bool, time::Instant, GenericError)>,

    keyboard_input: joypad::InputBtn,
    gamepad_input: joypad::InputBtn,
    mouse_pos: (isize, isize),

    current_rom_path: Option<PathBuf>,
    current_rom_header: rom::RomData,

    monitor_refresh_rate: usize,
    // fps: f32,
    // used only when audio is disabled
    video_sync_frame: f32,
    video_sync_ratio: f32,

    frame_number: usize,
    emulation: EmulationState,
}

fn ring_push_front<T: PartialEq>(queue: &mut VecDeque<T>, val: T, limit: usize) {
    // remove duplicate
    if let Some(idx) = queue.iter().position(|x| *x == val) {
        queue.remove(idx);
    }

    queue.push_front(val);
    queue.truncate(limit);
}

fn file_dialog(prompt: &str, requires: &str, extensions: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_can_create_directories(true)
        .set_title(prompt)
        .add_filter(requires, extensions)
        .pick_file()
}

struct AppCtx {
    // sdl: SdlCtx,
    emu: Arc<Mutex<NesEmulator>>,
    // emu_thread: thread::JoinHandle<()>,
    // is_running: Arc<atomic::AtomicBool>,
    //
    // video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
    tex: Arc<Mutex<egui::TextureHandle>>,

    audio: AudioHandler,
    gamepads: GamepadHandler,

    state: AppState,
    cfg: AppCfg,
}

impl AppCtx {
    pub fn new(c: &eframe::CreationContext) -> Box<Self> {
        let refresh_rate = c
            .winit_window()
            .and_then(|window| window.current_monitor())
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .and_then(|refresh_rate| Some(refresh_rate / 1000))
            .unwrap_or(60);

        #[cfg(not(feature = "persistence"))]
        let cfg = AppCfg {
            volume: 0.5,
            ..Default::default()
        };

        #[cfg(feature = "persistence")]
        let cfg = if let Some(storage) = c.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            AppCfg {
                volume: 0.5,
                ..Default::default()
            }
        };

        let img = egui::ColorImage::filled([256, 240], egui::Color32::TRANSPARENT);
        let tex = c.egui_ctx.load_texture("emu_present", img, TEX_OPTS);
        let tex = Arc::new(Mutex::new(tex));

        let emu = NesEmulator::empty();
        let emu = Arc::new(Mutex::new(emu));
        // let sdl = SdlCtx::new(44100, Arc::clone(&emu));

        // let video_chain = Arc::new(Mutex::new(RingBuffer::new(8)));

        let audio = AudioHandler::new(48000, 1024, Arc::clone(&emu), !cfg.disable_audio);
        emu.lock().unwrap().set_audio_rate(audio.buffer_size() as f32);

        // let samples_needed = audio.buffer_size();

        // let emu_arc = Arc::clone(&emu);
        // let chain_arc = Arc::clone(&video_chain);

        // let is_running = Arc::new(atomic::AtomicBool::new(false));
        // let is_running_arc = Arc::clone(&is_running);

        // let emu_thread = if audio.is_supported() {
        //     thread::Builder::new()
        //         .name("emulation".into())
        //         .spawn(move || {
        //             emulation_thread_proc(
        //                 emu_arc,
        //                 chain_arc,
        //                 samples_needed as usize,
        //                 is_running_arc,
        //             )
        //         })
        //         .unwrap()
        // } else {
        //     thread::Builder::new()
        //         .name("emulation".into())
        //         .spawn(move || emulation_thread_no_audio_proc(emu_arc, chain_arc, is_running_arc))
        //         .unwrap()
        // };

        let mut res = Self {
            // sdl,
            emu,
            // emu_thread,
            // is_running,
            // video_chain,
            tex,
            audio,

            gamepads: GamepadHandler::default(),

            cfg,
            state: Default::default(),
        };

        res.state.monitor_refresh_rate = refresh_rate as usize;
        res.update_fps();

        Box::new(res)
    }

    fn emu_lock(&self) -> MutexGuard<'_, NesEmulator> {
        self.emu.lock().unwrap()
    }

    fn update_fps(&mut self) {
        // if self.audio.is_enabled() {
        //     self.state.fps = self.cfg.refresh_rate.fps();
        //     println!("FPS updated to sync audio: {}", self.state.fps);
        // } else {
        //     let fps = 1.0 / self.emu_lock().region().frame_rate();
        //     self.state.fps = fps;
        //     println!("FPS updated to sync video: {fps}");
        // }

        let ratio = self.state.monitor_refresh_rate as f32 / self.emu_lock().region().frame_rate();
        self.state.video_sync_ratio = ratio;
    }

    fn add_message(&mut self, e: GenericError) {
        self.state.message_open = Some((true, time::Instant::now(), e));
    }

    fn open_dialog(
        &mut self,
        prompt: &str,
        requires: &str,
        extensions: &[&str],
    ) -> Option<PathBuf> {
        self.audio.pause();
        let res = file_dialog(prompt, requires, extensions);

        if self.state.emulation == EmulationState::Running {
            self.audio.resume();
        }
        res
    }

    fn load_palette<P: AsRef<Path>>(&mut self, path: P) {
        let res = fs::read(path)
            .map(|bytes| {
                NesPalette::from_pal_file(&bytes)
                    .ok_or("not a valid NES palette file")
                    .map(|pal| ring_push_front(&mut self.cfg.palettes, pal, 20))
                    .map(|_| self.add_message("palette loaded".into()))
            })
            .map_err(|e| self.add_message(e.into()));

        if res.is_ok() {
            if let Some(pal) = self.cfg.palettes.front() {
                self.emu_lock().palette = pal.clone();
            }
        }
    }

    fn close_and_save_rom_if_open(&mut self) {
        if self.state.emulation == EmulationState::Stopped {
            return;
        }

        if self.cfg.battery_save_enabled {
            if let Some(path) = &self.state.current_rom_path {
                let res = self.emu_lock().save_battery_to_file(path);

                if let Err(e) = res {
                    self.add_message(e.into());
                }
            }
        }

        #[cfg(feature = "savestates")]
        if self.cfg.restore_session {
            self.save_state("last");
        }
    }

    fn load_rom<P: AsRef<Path>>(&mut self, rom_path: P, force_reset: bool) {
        let bios = self
            .cfg
            .bios_path
            .as_ref()
            .and_then(|path| buffered_read(path).ok());

        let res = NesEmulator::load_rom_from_file(&rom_path, bios);
        match res {
            Ok(mut new_emu) => {
                self.close_and_save_rom_if_open();

                if self.cfg.battery_save_enabled {
                    if let Err(e) = new_emu.load_battery_from_file(&rom_path) {
                        self.add_message(e);
                    }
                }

                new_emu.update_settings(self.cfg.nes_settings.clone());
                if let Some(pal) = self.cfg.palettes.front() {
                    new_emu.palette = pal.clone();
                }

                self.state.current_rom_header = {
                    let mut emu = self.emu_lock();
                    *emu = new_emu;
                    emu.rom_info().clone()
                };

                let pathbuf = rom_path.as_ref().to_path_buf();
                self.state.current_rom_path = Some(pathbuf.clone());
                ring_push_front(&mut self.cfg.recent_roms, pathbuf, 12);

                #[cfg(feature = "savestates")]
                if self.cfg.restore_session && !force_reset {
                    self.load_state("last");
                }

                self.update_fps();
                self.state.emulation = match self.state.emulation {
                    EmulationState::Stopped | EmulationState::Running => {
                        self.audio.resume();
                        EmulationState::Running
                    }
                    EmulationState::Paused => {
                        self.audio.pause();
                        EmulationState::Paused
                    }
                };
            }

            Err(e) => {
                self.add_message(e);
            }
        }
    }

    fn reset_rom(&mut self) {
        if let Some(rom_path) = &self.state.current_rom_path {
            self.load_rom(rom_path.clone(), true);
        }
    }

    fn show_menubar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |content| {
            content.horizontal_wrapped(|ui| {
                ui.menu_button("💾 File", |ui| {
                    if ui.button("📂 Open...").clicked() {
                        self.open_dialog(
                            "Select game ROM",
                            "NES ROM",
                            &["nes", "fds", "zip", "rar"],
                        )
                        .map(|path| self.load_rom(path, false));
                    }

                    ui.menu_button("Recent ROMs", |ui| {
                        if self.cfg.recent_roms.is_empty() {
                            ui.label("No recent ROMs");
                            return;
                        }

                        for (i, entry) in self.cfg.recent_roms.iter().enumerate() {
                            if ui.button(entry.to_str().unwrap_or_default()).clicked() {
                                let to_load = self.cfg.recent_roms.remove(i).unwrap();
                                self.load_rom(to_load, false);
                                break;
                            }
                        }

                        if ui.button("Clear").clicked() {
                            self.cfg.recent_roms.clear();
                        }
                    });

                    #[cfg(feature = "savestates")]
                    ui.add_enabled_ui(self.state.emulation != EmulationState::Stopped, |ui| {
                        ui.menu_button("Savestates", |ui| {
                            if ui.button("Quicksave").clicked() {
                                self.save_state("quick");
                            }

                            if ui.button("Quickload").clicked() {
                                self.load_state("quick");
                            }

                            ui.separator();

                            ui.menu_button("Save Slots...", |ui| {
                                if ui.button("To file...").clicked() {
                                    // TODO: show file modal to save state
                                }

                                ui.separator();
                                for i in 1..9 {
                                    if ui.button(format!("Slot {i}")).clicked() {
                                        self.save_state(&i.to_string());
                                    }
                                }
                            });

                            ui.menu_button("Load Slots...", |ui| {
                                if ui.button("From file...").clicked() {
                                    // TODO: show file modal to load state
                                }

                                ui.separator();
                                for i in 1..9 {
                                    if ui.button(format!("Slot {i}")).clicked() {
                                        self.load_state(&i.to_string());
                                    }
                                }
                            });

                            #[cfg(target_os = "windows")]
                            if ui.button("Open states directory to clipboard").clicked() {
                                use std::process;

                                match process::Command::new("explorer.exe")
                                    .arg(self.get_states_dir())
                                    .spawn()
                                {
                                    Err(e) => self.add_message(e.into()),
                                    _ => {}
                                }
                            }

                            // #[cfg(not(target_os = "windows"))]
                            if ui.button("Copy states directory to clipboard").clicked() {
                                ui.copy_text(
                                    self.get_rom_states_dir()
                                        .into_os_string()
                                        .into_string()
                                        .unwrap_or_default(),
                                );
                            }

                            ui.separator();

                            if ui.button("🗑 Clear game states").clicked() {
                                // TODO: show modal
                                _ = std::fs::remove_dir_all(self.get_rom_states_dir());
                            }

                            if ui.button("☠ Clear all states").clicked() {
                                // TODO: show modal
                                let dir = self.get_states_dir();
                                _ = std::fs::remove_dir_all(dir);
                            }
                        })
                    });

                    if ui.button("📷 Screenshot").clicked() {
                        // TODO
                        eprintln!("screenshots not implemented yet");
                    }

                    if ui.button("❌ Close").clicked() {
                        self.state.exit_modal_open = true;
                    }
                });

                ui.menu_button("🕹 Emulation", |ui| {
                    match self.state.emulation {
                        EmulationState::Stopped => {
                            ui.add_enabled(false, egui::Button::new("▶ Run"));
                            ui.separator();
                            ui.add_enabled(false, egui::Button::new("🔄 Reset"));
                            ui.add_enabled(false, egui::Button::new("⏹ Stop"));
                        }

                        EmulationState::Paused => {
                            let run = ui.button("▶ Run");
                            ui.separator();
                            let reset = ui.button("🔄 Reset");
                            let stop = ui.button("⏹ Stop");

                            if run.clicked() {
                                self.state.emulation = EmulationState::Running;
                            } else if reset.clicked() {
                                self.reset_rom();
                            } else if stop.clicked() {
                                self.state.emulation = EmulationState::Stopped;
                            }
                        }

                        EmulationState::Running => {
                            let pause = ui.button("⏸ Pause");
                            ui.separator();
                            let reset = ui.button("🔄 Reset");
                            let stop = ui.button("⏹ Stop");

                            if pause.clicked() {
                                self.state.emulation = EmulationState::Paused;
                            } else if reset.clicked() {
                                self.reset_rom();
                            } else if stop.clicked() {
                                self.state.emulation = EmulationState::Stopped;
                            }
                        }
                    }

                    match self.state.emulation {
                        EmulationState::Running => self.audio.resume(),
                        _ => self.audio.pause(),
                    }

                    let mut emu = self.emu_lock();
                    let header = emu.rom_info();
                    if header.format == rom::HeaderFormat::Fds {
                        ui.separator();
                        if ui.button("💿 Insert next FDS disk/side").clicked() {
                            emu.mapper.special_input();
                        }
                    }
                });

                let running = self.state.emulation == EmulationState::Running;

                ui.menu_button("⚙ Settings", |ui| {
                    if ui.button("🔧 Emulator").clicked() {
                        self.state.settings_open = true;
                    }

                    if ui.button("🎮 Keybinds").clicked() {
                        self.state.keybinds_open = true;
                    }

                    ui.checkbox(&mut self.cfg.keep_aspect_ratio, "📺 Keep Aspect Ratio");
                    if ui
                        .checkbox(&mut self.cfg.fullscreen, "🖥 Fullscreen")
                        .clicked()
                    {
                        ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                            self.cfg.fullscreen,
                        ));
                    }
                    ui.checkbox(&mut self.cfg.hide_cursor, "🖱 Hide Cursor when playing");

                    // ui.menu_button("🖥 Video Size", |ui| {
                    //   for i in 1..6 {
                    //     if ui.radio_value(&mut self.cfg.video_size, i, format!("{i}x")).clicked() {
                    //       should_resize = Some(i);
                    //     }
                    //   }
                    // });

                    ui.menu_button("🎨 Theme", egui::widgets::global_theme_preference_buttons);
                });

                ui.menu_button("🐞 Debug", |ui| {
                    let rom_info = egui::Button::new("💾 ROM information");

                    if ui.add_enabled(running, rom_info).clicked() {
                        self.state.rom_info_open = true;
                    }

                    if ui.button("👢 Run FDS BIOS").clicked() {
                        match &self.cfg.bios_path {
                            Some(bios_path) => {
                                let bios = buffered_read(bios_path);

                                match bios {
                                    Ok(bios) => {
                                        let new_emu = NesEmulator::load_bios_only(Some(bios));

                                        match new_emu {
                                            Ok(new_emu) => {
                                                self.close_and_save_rom_if_open();
                                                *self.emu_lock() = new_emu;
                                                self.state.current_rom_header = rom::RomData {
                                                    title: "FDS BIOS".to_string(),
                                                    ..Default::default()
                                                };
                                                self.state.emulation = EmulationState::Running;
                                            }
                                            Err(e) => self.add_message(e),
                                        }
                                    }

                                    Err(e) => self.add_message(e),
                                }
                            }

                            None => self.add_message("no BIOS ROM provided".into()),
                        }
                    }
                });

                ui.menu_button("❔ Help", |ui| {
                    if ui.button("ℹ About").clicked() {
                        self.state.about_open = true;
                    }
                    ui.hyperlink("🛠 Report bugs, issues or features");
                });

                if self.state.emulation != EmulationState::Stopped {
                    let style = ui.style_mut();
                    style.spacing.slider_width *= 0.7;

                    ui.separator();
                    ui.label("🔊 Vol");

                    let volume_slider = egui::Slider::new(&mut self.cfg.volume, 0.0..=4.0);
                    ui.add(volume_slider);
                }
            });
        });
    }

    fn show_exit_dialog(&mut self, ui: &mut egui::Ui) {
        if self.state.exit_modal_open {
            egui::Modal::new(egui::Id::new("❌ Close")).show(ui, |ui| {
                ui.heading("❌ Closing emulator..");
                ui.label("Are you sure??");

                ui.horizontal(|ui| {
                    if ui.button("Yes").clicked() {
                        self.close_and_save_rom_if_open();

                        self.state.should_close = true;
                        ui.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("No").clicked() {
                        self.state.should_close = false;
                        self.state.exit_modal_open = false;
                    }
                });
            });
        }
    }

    fn show_settings_window(&mut self, ui: &mut egui::Ui) {
        let mut should_update_palette = None;
        let audio_disabled = self.cfg.disable_audio;
        let mut settings_open = self.state.settings_open;

        egui::Window::new("🔧 Settings")
            .collapsible(true)
            .resizable([true, true])
            .open(&mut settings_open)
            .show(ui, |ui| {
                if ui.button("🎨 Load palette file...").clicked() {
                    should_update_palette = self.open_dialog("Select a NES palette file", "NES PAL file", &["pal"]);
                }

                let settings = &mut self.cfg.nes_settings;

                ui.collapsing(" Misc", |ui| {
                    ui.checkbox(&mut self.cfg.battery_save_enabled, "Enable battery saving")
                    .on_hover_text("This will dump work RAM in the same directory as the ROM's.");

                    #[cfg(feature = "savestates")]
                    ui.checkbox(&mut self.cfg.restore_session, "Automatically restore last session when a game is reopened later");

                    ui.checkbox(&mut settings.random_ram, "Enable randomized RAM at startup")
                    .on_hover_text("Some games (such as Final Fantasy) use the random state of RAM at boot to seed their rngs");
                });

                ui.collapsing("📺 Video", |ui| {
                    ui.checkbox(&mut settings.disable_sprite_limit, "Show more than 8 sprites per scaline")
                    .on_hover_text("Reduces flickering, but may show glitches in some games");
                    ui.checkbox(&mut settings.enable_oam_read, "Enable fully emulated OAM read")
                    .on_hover_text("Fully emulates OAM read with its quirks, might decrease performance");
                    ui.checkbox(&mut settings.enable_background, "Enable background tiles");
                    ui.checkbox(&mut settings.enable_sprites, "Enable sprite tiles");
                    ui.checkbox(&mut settings.pal_borders, "Show side PAL black borders (unimplemented)");

                    // if self.audio.is_enabled() {
                    //     ui.label("Video refresh rate:").on_hover_text("Higer video refresh rate might reduce input latency");
                    //     ui.indent("Refresh rates", |ui| {
                    //         ui.radio_value(&mut self.cfg.refresh_rate, RefreshRate::Fps60, "60fps");
                    //         ui.radio_value(&mut self.cfg.refresh_rate, RefreshRate::Fps120, "120fps");
                    //         ui.radio_value(&mut self.cfg.refresh_rate, RefreshRate::Fps144, "144fps");
                    //     });
                    //     self.state.fps = self.cfg.refresh_rate.fps();
                    // }

                });
                ui.collapsing("🔊 Audio", |ui| {
                    ui.checkbox(&mut self.cfg.disable_audio, "Disable audio and drive emulation by video")
                    .on_hover_text("By driving emulation with video, we get better good pacing and no skipped frames");

                    if self.audio.is_enabled() {
                        ui.label("Audio sample rate:");
                        ui.indent("Sample rates", |ui| {
                            use nenemu_core::emu::SampleRate;
                            ui.radio_value(&mut self.cfg.sample_rate, SampleRate::Hz32000, "32000hz");
                            ui.radio_value(&mut self.cfg.sample_rate, SampleRate::Hz44100, "44100hz");
                            ui.radio_value(&mut self.cfg.sample_rate, SampleRate::Hz48000, "48000hz");
                            ui.radio_value(&mut self.cfg.sample_rate, SampleRate::Hz96000, "96000hz");
                        });

                        ui.checkbox(&mut settings.enable_pulse0, "Enable pulse 0 channel");
                        ui.checkbox(&mut settings.enable_pulse1, "Enable pulse 1 channel");
                        ui.checkbox(&mut settings.enable_triangle, "Enable triangle channel");
                        ui.checkbox(&mut settings.enable_noise, "Enable noise channel");
                        ui.checkbox(&mut settings.enable_dmc, "Enable dmc channel");
                        ui.checkbox(&mut settings.enable_ext_audio, "Enable external sound chip");
                    }
                });

                ui.collapsing("💿 Famicon Disk System (FDS)", |ui| {
                    let bios_btn_text = if let Some(path) = &self.cfg.bios_path {
                    format!("👢 BIOS selected at: {:?}, click to change...", path)
                    } else {
                        "👢 Load FDS BIOS file...".to_string()
                    };

                    if ui.button(bios_btn_text).clicked() {
                        self.open_dialog("Select FDS BIOS file", "FDS BIOS", &["rom"])
                            .map(|path| self.cfg.bios_path = Some(path));
                    }

                    // TODO: disk handling
                })
            });

        self.state.settings_open = settings_open;

        if audio_disabled != self.cfg.disable_audio {
            self.audio.set_enabled(!self.cfg.disable_audio, Arc::clone(&self.emu));
            self.update_fps();
        }

        {
            let mut emu = self.emu_lock();
            if self.cfg.nes_settings != emu.settings {
                emu.update_settings(self.cfg.nes_settings.clone());
            }
        }

        if let Some(pal_path) = should_update_palette {
            self.load_palette(pal_path);
        }
    }

    fn show_keybids_window(&mut self, ui: &mut egui::Ui) {
        // TODO: controller keybindings

        egui::Window::new("🎮 Keybindings")
            .collapsible(true)
            .resizable([true, true])
            .open(&mut self.state.keybinds_open)
            .show(ui, |ui| {
                for (key, emu_btn) in &self.cfg.keymaps.keys {
                    ui.columns_const::<2, _>(|ui| {
                        let btn_name = &self.cfg.keymaps.btns_strings[emu_btn];
                        let col_src = ui[0].label(btn_name);
                        let col_dst = ui[1].button(format!("{:?}", key));

                        if let Some((rebind_key, _)) = &self.cfg.keymaps.rebind_key {
                            if rebind_key == key {
                                col_src.highlight();
                                col_dst.highlight();
                            } else if col_dst.clicked() {
                                self.cfg.keymaps.rebind_key = Some((*key, *emu_btn));
                            }
                        } else if col_dst.clicked() {
                            self.cfg.keymaps.rebind_key = Some((*key, *emu_btn));
                        }
                    });
                }

                ui.vertical_centered(|ui| {
                    if let Some(rebind_key) = &self.cfg.keymaps.rebind_key {
                        ui.label(format!(
                            "Rebinding {:?}... Press any button, close window to cancel",
                            rebind_key.1
                        ));
                    }
                });

                ui.set_clip_rect(ui.min_rect());
            })
            .or_else(|| {
                self.cfg.keymaps.rebind_key = None;
                None
            });

        // TODO: gamepad rebinds
    }

    fn show_rom_info_window(&mut self, ui: &mut egui::Ui) {
        let header = &self.state.current_rom_header;

        egui::Window::new("💾 ROM information")
            .collapsible(true)
            .open(&mut self.state.rom_info_open)
            .show(ui, |ui| {
                ui.columns_const::<2, _>(|ui| {
                    ui[0].label("Game Title");
                    ui[1].label(&header.title);

                    ui[0].label("Header kind");
                    ui[1].label(format!("{:?}", header.format));

                    // TODO: more mapper information
                    ui[0].label("Mapper ID");
                    ui[1].label(header.mapper.to_string());
                    ui[0].label("SubMapper ID");
                    ui[1].label(header.submapper.to_string());

                    ui[0].label("Region");
                    ui[1].label(format!("{:?}", header.region));

                    ui[0].label("PRG size");
                    ui[1].label(format!("{} KB", header.prg_size / 1024));
                    ui[0].label("WRAM size");
                    ui[1].label(format!("{} KB", header.wram_size / 1024));

                    ui[0].label("CHR size");
                    ui[1].label(format!("{} KB", header.chr_size / 1024));
                    ui[0].label("CHR RAM");
                    ui[1].label(header.has_chr_ram.to_string());

                    ui[0].label("Battery");
                    ui[1].label(header.has_battery.to_string());
                });

                ui.set_clip_rect(ui.min_rect());
            });
    }

    fn show_about_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("ℹ About")
            .collapsible(true)
            .open(&mut self.state.about_open)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    // TODO: do richtext shit
                    ui.hyperlink_to("Nen Emulator", "https://github.com/Comba92/nen-emulator");
                    ui.label("Developed by");
                    ui.hyperlink_to("Comba92", "https://github.com/Comba92");
                    ui.hyperlink_to(
                        "Report bugs or issues",
                        "https://github.com/Comba92/nen-emulator/issues/new/choose",
                    );

                    ui.set_clip_rect(ui.min_rect());
                })
            });
    }

    fn show_error_window(&mut self, ui: &mut egui::Ui) {
        if let Some((open, appeared, msg)) = &mut self.state.message_open {
            let res = egui::Window::new("Message")
                .anchor(egui::Align2::LEFT_TOP, [10.0, 30.0])
                .title_bar(false)
                .collapsible(false)
                .auto_sized()
                .fade_in(true)
                .fade_out(true)
                .open(open)
                .show(ui, |ui| {
                    ui.heading(msg.to_string());
                });

            const MSG_DELAY: time::Duration = time::Duration::from_secs(4);
            if appeared.elapsed() > MSG_DELAY {
                *open = false;
            }

            if let None = res {
                self.state.message_open = None;
            }
        }
    }

    fn handle_input_and_emulation(&mut self, ui: &mut egui::Ui) {
        let current_input = self.emu_lock().get_buttons();

        let keyboard_input = ui.input(|i| {
            let mut pressed = joypad::InputBtn::empty();

            for (key, emu_btn) in &self.cfg.keymaps.keys {
                pressed.set(*emu_btn, i.key_down(*key));
            }

            pressed
        });

        let mouse_clicked = ui.input(|i| {
            i.pointer.any_down()
                && matches!(self.state.mouse_pos.0, 0..256)
                && matches!(self.state.mouse_pos.1, 0..240)
        });

        ui.input(|i| {
            if i.viewport().minimized.filter(|x| *x).is_some() {
                self.audio.pause();
            } else if i.viewport().maximized.filter(|x| *x).is_some() {
                self.audio.resume();
            }
        });

        let gamepad_input = self.gamepads.poll(current_input, &self.cfg.keymaps);

        if self.state.emulation == EmulationState::Running {
            {
                let mut emu = self.emu_lock();

                if keyboard_input != self.state.keyboard_input {
                    emu.set_buttons_all(keyboard_input);
                } else if gamepad_input != self.state.gamepad_input {
                    emu.set_buttons_all(gamepad_input);
                }

                emu.set_zapper_trigger(mouse_clicked);
                emu.set_zapper_light(self.state.mouse_pos.0, self.state.mouse_pos.1);

                // while emu.audio_queued() < 1024 * 2 {
                //     emu.step()
                // }

                if !self.audio.is_enabled()
                    && self.state.video_sync_frame >= self.state.video_sync_ratio
                {
                    _ = emu.step_until_frame_ready();
                }

                match emu.check_for_errrors() {
                    Ok(_) => {
                        if emu.frame_number() != self.state.frame_number {
                            let framebuf = egui::ColorImage::from_rgba_unmultiplied(
                                [256, 240],
                                emu.get_video_rgba(),
                            );
                            // self.video_chain.lock().unwrap().push(framebuf);
                            let frame_number = emu.frame_number();

                            drop(emu);
                            self.state.frame_number = frame_number;
                            self.tex.lock().unwrap().set(framebuf, TEX_OPTS);
                        }
                    }

                    Err(e) => {
                        drop(emu);
                        self.state.emulation = EmulationState::Stopped;
                        self.add_message(e.into());
                    }
                }
            }

            if !self.audio.is_enabled() {
                if self.state.video_sync_frame >= self.state.video_sync_ratio {
                    self.state.video_sync_frame -= self.state.video_sync_ratio;
                }

                self.state.video_sync_frame += 1.0;
            }

            self.state.keyboard_input = keyboard_input;
            self.state.gamepad_input = gamepad_input;
        }
    }
}

impl eframe::App for AppCtx {
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.cfg);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menubar")
            .show_separator_line(true)
            .show_inside(ui, |ui| self.show_menubar(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                egui::Frame::new().show(ui, |ui| {
                    let tex = self.tex.lock().unwrap();
                    let img = egui::Image::new(&*tex)
                        .maintain_aspect_ratio(self.cfg.keep_aspect_ratio)
                        .fit_to_exact_size(ui.max_rect().size());

                    let screen = ui.add(img);
                    self.state.mouse_pos = ui.input(|i| {
                        let abs_pos = i.pointer.latest_pos().unwrap_or_default();
                        let rel_pos = abs_pos - screen.rect.min;
                        let nes_pos = (
                            (rel_pos.x * 256.0) / screen.rect.width(),
                            (rel_pos.y * 240.0) / screen.rect.height(),
                        );
                        (nes_pos.0.round() as isize, nes_pos.1.round() as isize)
                    });

                    if self.state.emulation == EmulationState::Running && self.cfg.hide_cursor {
                        screen.on_hover_cursor(egui::CursorIcon::None);
                    }
                });
            });
        });

        ui.input(|i| {
            // check for dropped files
            let files = &i.raw.dropped_files;
            if let Some(Some(path)) = files.first().map(|f| &f.path) {
                let pal_ext = std::ffi::OsStr::new("pal");
                if path.extension() == Some(pal_ext) {
                    self.load_palette(path);
                } else {
                    self.load_rom(path, false);
                }
            }
        });

        // {
        //     let mut emu_lock = self.emu.lock().unwrap();
        //     while emu_lock.audio_queued() < self.sdl.samplebuf_size {
        //         emu_lock
        //             .step_until_samples_or_frame_ready(self.sdl.samplebuf_size * 2)
        //             .unwrap();

        //         if emu_lock.is_frame_ready() {
        //             let framebuf = egui::ColorImage::from_rgba_unmultiplied(
        //                 [256, 240],
        //                 emu_lock.get_video_rgba(),
        //             );
        //             self.video_chain.lock().unwrap().push(framebuf);
        //         }
        //     }
        // }

        self.show_settings_window(ui);
        self.show_keybids_window(ui);
        self.show_about_window(ui);
        self.show_exit_dialog(ui);
        self.show_rom_info_window(ui);
        self.show_error_window(ui);

        self.handle_input_and_emulation(ui);

        // if self.state.emulation != current_state {
        // self.is_running.store(
        //     self.state.emulation == EmulationState::Running,
        //     atomic::Ordering::Relaxed,
        // );
        // }

        // {
        //     let mut video_lock = self.video_chain.lock().unwrap();
        //     if video_lock.queued() > 0 {
        //         let framebuf = std::mem::take(video_lock.pop_mut());
        //         self.tex.lock().unwrap().set(framebuf, TEX_OPTS);
        //     }
        // }

        *self.audio.volume.lock().unwrap() = self.cfg.volume;

        const FPS: f32 = 1.0 / 120.0;
        ui.request_repaint_after_secs(FPS);
        // ui.request_repaint_after_secs(self.state.fps);

        if ui.input(|i| i.viewport().close_requested()) {
            if !self.state.should_close {
                ui.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.state.exit_modal_open = true;
            }
        }
    }
}

#[cfg(feature = "savestates")]
impl AppCtx {
    fn get_states_dir(&self) -> PathBuf {
        let mut dir = eframe::storage_dir(APP_NAME).unwrap();
        dir.push("states");
        dir
    }

    fn get_rom_states_dir(&self) -> PathBuf {
        let mut dir = self.get_states_dir();
        let current_rom = self.state.current_rom_path.as_ref().unwrap();
        dir.push(current_rom.file_stem().unwrap());
        dir
    }

    fn save_state(&mut self, name: &str) {
        let mut dir = self.get_states_dir();

        let current_rom = self.state.current_rom_path.as_ref().unwrap();
        dir.push(current_rom.file_stem().unwrap());

        _ = std::fs::create_dir_all(&dir);

        dir.push(name);
        dir.set_extension("state");
        let res = self.emu_lock().savestate(dir);
        match res {
            Err(e) => self.add_message(e),
            _ => {}
        }
    }

    fn load_state(&mut self, name: &str) {
        let mut dir = self.get_rom_states_dir();

        dir.push(name);
        dir.set_extension("state");
        _ = self.emu_lock().loadstate(dir);
    }
}
