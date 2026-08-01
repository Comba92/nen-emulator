// this removes the windows console
// #![windows_subsystem = "windows"]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui::{self, Key::P};

use nenemu_core::{
    NesPalette,
    emu::NesEmulator,
    joypad,
    rom::{self, is_valid_bios},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, mpsc},
    thread,
    time::{self, Duration, Instant},
};

const TEX_OPTS: egui::TextureOptions = egui::TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Nearest,
    wrap_mode: egui::TextureWrapMode::ClampToEdge,
    mipmap_mode: Some(egui::TextureFilter::Nearest),
};

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy)]
enum EmulatorAction {
    Reset,
    TogglePause,
    ToggleMute,
    #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
    Savestate,
    #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
    Loadstate,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy)]
enum EmulatorInput {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
enum PlayerInput {
    Joypad(joypad::JoypadInput),
    Action(EmulatorAction),
}

impl std::fmt::Display for PlayerInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Joypad(joy) => write!(f, "{:?}", joy),
            Self::Action(act) => write!(f, "{:?}", act),
        }
    }
}

struct Binding {
    name: String,
    kind: PlayerInput,
    keys: Vec<egui::Key>,
    btns: Vec<gilrs::Button>,
}
impl Binding {
    pub fn new(kind: PlayerInput, keys: Vec<egui::Key>, btns: Vec<gilrs::Button>) -> Self {
        Self {
            name: kind.to_string(),
            kind,
            keys,
            btns,
        }
    }
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct InputHandler {
    binds: Vec<Binding>,
    rebind: Option<String>,
    keys_taken: HashSet<egui::Key>,
    btns_taken: HashSet<gilrs::Button>,
}

impl Default for InputHandler {
    fn default() -> Self {
        use EmulatorAction::*;
        use egui::Key;
        use gilrs::Button as Btn;
        use joypad::JoypadInput as Joy;

        let binds = vec![
            Binding::new(
                PlayerInput::Joypad(Joy::Up),
                vec![Key::ArrowUp],
                vec![Btn::DPadUp],
            ),
            Binding::new(
                PlayerInput::Joypad(Joy::Down),
                vec![Key::ArrowDown],
                vec![Btn::DPadDown],
            ),
            Binding::new(
                PlayerInput::Joypad(Joy::Left),
                vec![Key::ArrowLeft],
                vec![Btn::DPadLeft],
            ),
            Binding::new(
                PlayerInput::Joypad(Joy::Right),
                vec![Key::ArrowRight],
                vec![Btn::DPadRight],
            ),
            Binding::new(
                PlayerInput::Joypad(Joy::A),
                vec![Key::S],
                vec![Btn::South, Btn::East],
            ),
            Binding::new(
                PlayerInput::Joypad(Joy::B),
                vec![Key::A],
                vec![Btn::West, Btn::North],
            ),
            Binding::new(
                PlayerInput::Joypad(Joy::Start),
                vec![Key::W],
                vec![Btn::Start],
            ),
            Binding::new(
                PlayerInput::Joypad(Joy::Select),
                vec![Key::E],
                vec![Btn::Select],
            ),
            Binding::new(PlayerInput::Action(TogglePause), vec![Key::P], vec![]),
            Binding::new(PlayerInput::Action(ToggleMute), vec![Key::M], vec![]),
            Binding::new(PlayerInput::Action(Reset), vec![Key::R], vec![]),
            #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
            Binding::new(PlayerInput::Action(Savestate), vec![Key::Num0], vec![]),
            #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
            Binding::new(PlayerInput::Action(Loadstate), vec![Key::Num9], vec![]),
        ];

        let keys_taken = HashSet::from_iter(binds.iter().flat_map(|b| b.keys.clone()));
        let btns_taken = HashSet::from_iter(binds.iter().flat_map(|b| b.btns.clone()));

        Self {
            binds,
            rebind: None,
            keys_taken,
            btns_taken,
        }
    }
}

struct GamepadHandler {
    pub api: Option<gilrs::Gilrs>,
}

impl GamepadHandler {
    pub fn new() -> Self {
        let api = gilrs::Gilrs::new()
            .inspect_err(|err| eprintln!("{err}"))
            .ok();

        Self { api }
    }
}

fn cpal_callback(emu: &Arc<Mutex<NesEmulator>>, volume: &Arc<Mutex<f32>>, audio_out: &mut [f32]) {
    let mut emu_lock = emu.lock().unwrap();

    // hybrid approach: we sync by video, however we might not have enough samples to provide the callback. we can still step until we have some ready.
    // in the very common case, we'll step more into the vblank period. if we don't go way further and reach another vblank (we definetely won't), we won't have skipped frames
    while emu_lock.audio_queued() < audio_out.len() {
        emu_lock.step();
    }

    let volume = *volume.lock().unwrap();

    let samples = emu_lock.get_audio_f32_iter(audio_out.len() / 2);
    for (i, sample) in samples.enumerate() {
        audio_out[2 * i] = sample * volume;
        audio_out[2 * i + 1] = sample * volume;
    }
}

struct AudioStreamData {
    cb: cpal::Stream,
    device: cpal::Device,
    device_id: cpal::DeviceId,
    cfg: cpal::StreamConfig,
}
impl AudioStreamData {
    fn new(
        device: cpal::Device,
        buf_size: usize,
        cfg: cpal::SupportedStreamConfig,
        volume: &Arc<Mutex<f32>>,
        emu: &Arc<Mutex<NesEmulator>>,
    ) -> Option<Self> {
        let volume_arc = Arc::clone(volume);
        let emu_arc = Arc::clone(emu);
        let mut cfg = cfg.config();
        cfg.buffer_size = cpal::BufferSize::Fixed(buf_size as cpal::FrameCount);
        println!("{cfg:?}");

        device
            .build_output_stream(
                cfg,
                move |audio_out, _| cpal_callback(&emu_arc, &volume_arc, audio_out),
                |err| eprintln!("Cpal callback error: {err}"),
                None,
            )
            .inspect_err(|e| eprintln!("Cpal creation error {e}")) // TODO: poll for new output device here
            .ok()
            .and_then(|cb| {
                Some(AudioStreamData {
                    cb,
                    device_id: device.id().unwrap(),
                    device,
                    cfg: cfg.into(),
                })
            })
    }
}

fn cpal_query_cfgs(device: &cpal::Device) -> Option<cpal::SupportedStreamConfig> {
    device
        .default_output_config()
        .inspect_err(|err| eprintln!("{err}"))
        .ok()
        .filter(|cfg| {
            // check if default config is ok
            cfg.channels() == 2 && cfg.sample_format() == cpal::SampleFormat::F32
        })
        .or_else(|| {
            // search for a good config
            device
                .supported_output_configs()
                .inspect_err(|err| eprintln!("{err}"))
                .ok()
                .and_then(|cfgs| {
                    cfgs.filter(|cfg| {
                        cfg.channels() == 2 && cfg.sample_format() == cpal::SampleFormat::F32
                    })
                    .filter_map(|cfg| cfg.try_with_standard_sample_rate())
                    .next()
                })
        })
}

struct AudioHandler {
    host: cpal::Host,
    stream: Option<AudioStreamData>,
    buf_size: usize,
    enabled: bool,
    muted: bool,
    playing: bool,
    volume: Arc<Mutex<f32>>,
}

impl AudioHandler {
    pub fn new(emu: &Arc<Mutex<NesEmulator>>, enabled: bool, buf_size: usize) -> Self {
        let host = cpal::default_host();

        // take the default device for now
        match host.default_output_device() {
            Some(device) => {
                // println!("HOSTS: {:?}", cpal::available_hosts());
                // println!("HOST chosen: {:?}", host.id());
                // println!("DEV chosen: {device:?} {:?}", device.id());
                // println!("Default CFG: {:?}", device.default_output_config();

                let volume = Arc::new(Mutex::new(0.5));
                let stream = cpal_query_cfgs(&device)
                    .and_then(|cfg| AudioStreamData::new(device, buf_size, cfg, &volume, emu));

                let res = Self {
                    host,
                    stream,
                    buf_size,
                    muted: false,
                    playing: false,
                    enabled,
                    volume,
                };

                res
            }

            None => Self {
                host,
                stream: None,
                buf_size,
                muted: false,
                playing: false,
                enabled: false,
                volume: Default::default(),
            },
        }
    }

    pub fn current_device_id(&self) -> Option<&cpal::DeviceId> {
        self.stream.as_ref().map(|s| &s.device_id)
    }

    pub fn current_device(&self) -> Option<&cpal::Device> {
        self.stream.as_ref().map(|s| &s.device)
    }

    pub fn is_supported(&self) -> bool {
        self.stream.is_some()
    }

    pub fn is_enabled(&self) -> bool {
        self.is_supported() && self.enabled
    }

    pub fn sample_rate(&self) -> u32 {
        self.stream
            .as_ref()
            .map(|s| s.cfg.sample_rate)
            .unwrap_or_default()
    }

    pub fn buffer_size(&self) -> usize {
        self.stream
            .as_ref()
            .map(|s| s.cb.buffer_size().unwrap_or_default())
            .unwrap_or_default() as usize
    }

    pub fn set_enabled(&mut self, cond: bool) {
        self.enabled = cond;

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

    fn set_ouput_device(&mut self, device: cpal::Device, emu: &Arc<Mutex<NesEmulator>>) {
        let new_stream = cpal_query_cfgs(&device)
            .and_then(|cfg| AudioStreamData::new(device, self.buf_size, cfg, &self.volume, emu));

        if new_stream.is_some() {
            self.stream = new_stream;
        }

        if self.playing {
            self.resume();
        }
    }

    pub fn resume(&mut self) {
        if !self.enabled {
            return;
        }

        self.playing = true;
        match &self.stream {
            Some(stream) => _ = stream.cb.play(),
            _ => {}
        }
    }

    pub fn pause(&mut self) {
        if !self.enabled {
            return;
        }

        self.playing = false;
        match &self.stream {
            Some(stream) => _ = stream.cb.pause(),
            _ => {}
        }
    }
}

// https://github.com/giroletm/SDL2_gfx/blob/master/SDL2_framerate.c
struct FpsHandler {
    clock: Instant,
    frame_count: usize,
    rate_ticks: f32,
    base_ticks: u64,
    last_ticks: u64,
}
impl FpsHandler {
    pub fn new(fps: usize) -> Self {
        // TODO: wasm doesn't have access to time
        let clock = Instant::now();
        let ticks = clock.elapsed().as_millis();
        Self {
            clock,
            frame_count: 0,
            rate_ticks: 1000.0 / fps as f32,
            base_ticks: ticks as u64,
            last_ticks: ticks as u64,
        }
    }

    pub fn set_framerate(&mut self, fps: usize) {
        self.frame_count = 0;
        self.rate_ticks = 1000.0 / fps as f32;
    }

    pub fn delay(&mut self) {
        self.frame_count += 1;
        let current_ticks = self.clock.elapsed().as_millis() as u64;
        self.last_ticks = current_ticks;

        let target_ticks = self.base_ticks + (self.frame_count as f32 * self.rate_ticks) as u64;
        if current_ticks <= target_ticks {
            thread::sleep(Duration::from_millis((target_ticks - current_ticks)));
        } else {
            self.frame_count = 0;
            self.base_ticks = self.clock.elapsed().as_millis() as u64;
        }
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

fn buffered_write<P: AsRef<Path>>(path: P, bytes: &[u8]) -> Result<(), GenericError> {
    use std::io::Write;

    let file = std::fs::File::open(path)?;
    let mut writer = std::io::BufWriter::new(file);
    writer.write_all(bytes)?;
    Ok(())
}

#[derive(Default, PartialEq, Clone, Copy, Debug)]
enum EmulationState {
    #[default]
    Stopped,
    Running,
    Paused,
}

const APP_NAME: &'static str = "NenEmu";
type GenericError = Box<dyn std::error::Error>;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let opts = eframe::NativeOptions {
        centered: true,
        persist_window: true,
        viewport: egui::ViewportBuilder::default()
            .with_drag_and_drop(true)
            .with_inner_size((640.0 * 2.0, 480.0 * 2.0))
            .with_title(APP_NAME),
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,

        vsync: false,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            present_mode: eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync,
            ..Default::default()
        },

        #[cfg(feature = "opengl")]
        renderer: eframe::Renderer::Glow,

        ..Default::default()
    };

    eframe::run_native(APP_NAME, opts, Box::new(|c| Ok(AppCtx::new(c)))).unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(|c| Ok(AppCtx::new(c))))
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct AppCfg {
    input: InputHandler,
    recent_roms: VecDeque<PathBuf>,
    palettes: VecDeque<NesPalette>,
    bios_path: Option<PathBuf>,
    keep_aspect_ratio: bool,
    fullscreen: bool,
    hide_cursor: bool,
    battery_save_enabled: bool,
    hide_exit_dialog: bool,

    #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
    restore_session: bool,

    nes_settings: nenemu_core::emu::NesSettings,

    disable_audio: bool,
    volume: f32,
}
impl AppCfg {
    fn new() -> Self {
        Self {
            volume: 0.5,
            keep_aspect_ratio: true,
            ..Default::default()
        }
    }
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

    keyboard_input: joypad::JoypadInput,
    gamepad_input: joypad::JoypadInput,
    mouse_pos: (isize, isize),

    bios: Option<Box<[u8]>>,
    current_rom: Option<(Box<[u8]>, PathBuf)>,
    current_rom_header: rom::RomData,

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

#[cfg(not(target_arch = "wasm32"))]
fn file_dialog(prompt: &str, requires: &str, extensions: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_can_create_directories(true)
        .set_title(prompt)
        .add_filter(requires, extensions)
        .pick_file()
}

enum FileOpenKind {
    NesRom,
    NesPalette,
    FdsBios,
}
struct FileDialogHandler {
    send: mpsc::Sender<(Vec<u8>, PathBuf, FileOpenKind)>,
    recv: mpsc::Receiver<(Vec<u8>, PathBuf, FileOpenKind)>,
}
impl FileDialogHandler {
    pub fn new() -> Self {
        let (send, recv) = mpsc::channel();
        Self { send, recv }
    }

    pub fn open_dialog(
        &self,
        kind: FileOpenKind,
        prompt: &str,
        requires: &str,
        extensions: &[&str],
    ) {
        let dialog = rfd::AsyncFileDialog::new()
            .set_can_create_directories(true)
            .set_title(prompt)
            .add_filter(requires, extensions)
            .pick_file();

        let send = self.send.clone();
        let future = async move {
            if let Some(file) = dialog.await {
                let bytes = file.read().await;

                #[cfg(target_arch = "wasm32")]
                send.send((bytes, file.file_name().into(), kind)).unwrap();

                #[cfg(not(target_arch = "wasm32"))]
                send.send((bytes, file.path().into(), kind)).unwrap();
            }
        };

        // simply run a thread and detach it. it doesnt have to do much work anyway
        #[cfg(not(target_arch = "wasm32"))]
        thread::spawn(move || futures::executor::block_on(future));

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(future);
    }
}

struct AppCtx {
    // sdl: SdlCtx,
    emu: Arc<Mutex<NesEmulator>>,
    tex: egui::TextureHandle,
    audio: AudioHandler,
    gamepads: GamepadHandler,
    file_dialog: FileDialogHandler,
    fps: FpsHandler,

    state: AppState,
    cfg: AppCfg,
}

impl AppCtx {
    pub fn new(c: &eframe::CreationContext) -> Box<Self> {
        #[cfg(not(feature = "persistence"))]
        let cfg = AppCfg::new();

        #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
        let cfg = if let Some(storage) = c.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            AppCfg::new()
        };

        let img = egui::ColorImage::filled([256, 240], egui::Color32::TRANSPARENT);
        let tex = c.egui_ctx.load_texture("emu_present", img, TEX_OPTS);

        let emu = NesEmulator::empty();
        let emu = Arc::new(Mutex::new(emu));

        let audio = AudioHandler::new(&emu, !cfg.disable_audio, 1024);
        let fps = FpsHandler::new(60);

        let mut res = Self {
            emu,
            tex,
            audio,
            gamepads: GamepadHandler::new(),
            file_dialog: FileDialogHandler::new(),
            fps,

            cfg,
            state: Default::default(),
        };

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(bios_path) = &res.cfg.bios_path {
            match buffered_read(bios_path) {
                Ok(bios) => res.state.bios = Some(bios.into_boxed_slice()),

                Err(_) => {
                    // bios was not found, clear cfg
                    res.cfg.bios_path = None;
                    res.add_message("BIOS path provided but was not found");
                }
            }
        }

        // res.state.monitor_refresh_rate = refresh_rate as usize;
        // res.update_video_sync_fps();

        Box::new(res)
    }

    fn emu_lock(&self) -> MutexGuard<'_, NesEmulator> {
        self.emu.lock().unwrap()
    }

    fn add_message<E: Into<GenericError>>(&mut self, e: E) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.state.message_open = Some((true, time::Instant::now(), e.into())); // wasm doesnt support time
        }
    }

    fn resume_emulation(&mut self) {
        self.state.emulation = EmulationState::Running;
        self.audio.resume();
    }

    fn pause_emulation(&mut self) {
        self.state.emulation = EmulationState::Paused;
        self.audio.pause();
    }

    fn stop_emulation(&mut self) {
        self.state.emulation = EmulationState::Stopped;
        self.audio.pause();

        // clear screen
        self.tex.set(
            egui::ColorImage::filled([256, 240], egui::Color32::TRANSPARENT),
            TEX_OPTS,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_dialog_blocking(
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

    fn load_palette_from_file<P: AsRef<Path>>(&mut self, path: P) {
        // let res = fs::read(path)
        //     .map(|bytes| {
        //         NesPalette::from_pal_file(&bytes)
        //             .ok_or("not a valid NES palette file")
        //             .map(|pal| ring_push_front(&mut self.cfg.palettes, pal, 20))
        //             .map(|_| self.add_message("palette loaded".into()))
        //     })
        //     .map_err(|e| self.add_message(e.into()));

        // if res.is_ok() {
        //     if let Some(pal) = self.cfg.palettes.front() {
        //         self.emu_lock().palette = pal.clone();
        //     }
        // }
        _ = fs::read(path)
            .and_then(|bytes| Ok(self.load_palette_from_bytes(&bytes)))
            .map_err(|e| self.add_message(e));
    }

    fn load_palette_from_bytes(&mut self, bytes: &[u8]) {
        let res = NesPalette::from_pal_file_bytes(&bytes).ok_or("not a valid NES palette file");

        match res {
            Ok(pal) => {
                self.emu_lock().set_palette(pal.clone());
                ring_push_front(&mut self.cfg.palettes, pal, 20);
                self.add_message("palette loaded");
            }
            Err(e) => self.add_message(e),
        }
    }

    fn close_and_save_rom_if_open(&mut self) {
        if self.state.emulation == EmulationState::Stopped {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.cfg.battery_save_enabled {
            if let Some((_, path)) = &self.state.current_rom {
                let res = self.emu_lock().save_battery_to_file(path);

                if let Err(e) = res {
                    self.add_message(e);
                }
            }
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "savestates"))]
        if self.cfg.restore_session {
            self.save_state("last");
        }
    }

    fn load_rom_from_bytes<P: AsRef<Path>>(
        &mut self,
        rom_path: P,
        rom_bytes: Box<[u8]>,
        _force_reset: bool,
    ) {
        match NesEmulator::builder()
            .with_rom(&rom_bytes)
            .with_fds_bios(self.state.bios.as_ref())
            .build()
        {
            Ok(new_emu) => self.load_rom(rom_path, rom_bytes, new_emu, _force_reset),
            Err(e) => self.add_message(e),
        }
    }

    fn load_rom_from_file<P: AsRef<Path>>(&mut self, rom_path: P, _force_reset: bool) {
        match buffered_read(&rom_path) {
            Ok(rom) => self.load_rom_from_bytes(rom_path, rom.into_boxed_slice(), _force_reset),
            Err(e) => self.add_message(e),
        }
    }

    fn load_rom<P: AsRef<Path>>(
        &mut self,
        rom_path: P,
        rom_bytes: Box<[u8]>,
        mut new_emu: NesEmulator,
        _force_reset: bool,
    ) {
        self.close_and_save_rom_if_open();

        #[cfg(not(target_arch = "wasm32"))]
        if self.cfg.battery_save_enabled {
            _ = new_emu.load_battery_from_file(&rom_path);
        }

        new_emu.set_settings(self.cfg.nes_settings.clone());
        new_emu.set_audio_rate(self.audio.sample_rate() as f64);
        self.fps.set_framerate(new_emu.frame_rate() as usize);

        if let Some(pal) = self.cfg.palettes.front() {
            new_emu.palette = pal.clone();
        }

        self.state.current_rom_header = {
            let mut emu = self.emu_lock();
            *emu = new_emu;
            emu.rom_info().clone()
        };

        let pathbuf = rom_path.as_ref().to_path_buf();
        self.state.current_rom = Some((rom_bytes, pathbuf.clone()));
        ring_push_front(&mut self.cfg.recent_roms, pathbuf, 12);

        #[cfg(all(not(target_arch = "wasm32"), feature = "savestates"))]
        if self.cfg.restore_session && !_force_reset {
            self.load_state("last");
        }

        // self.update_video_sync_fps();
        match self.state.emulation {
            EmulationState::Stopped | EmulationState::Running => self.resume_emulation(),
            EmulationState::Paused => self.pause_emulation(),
        };
    }

    fn reset_emulation(&mut self) {
        if let Some((rom, path)) = self.state.current_rom.take() {
            self.load_rom_from_bytes(path, rom, true);
        }
    }

    fn show_menubar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |content| {
            content.horizontal_wrapped(|ui| {
                ui.menu_button("💾 File", |ui| {
                    if ui.button("📂 Open...").clicked() {
                        self.file_dialog.open_dialog(
                            FileOpenKind::NesRom,
                            "Select game ROM",
                            "NES ROM",
                            &["nes", "fds", "zip", "rar"],
                        );
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    ui.menu_button("Recent ROMs", |ui| {
                        if self.cfg.recent_roms.is_empty() {
                            ui.label("No recent ROMs");
                            return;
                        }

                        for (i, entry) in self.cfg.recent_roms.iter().enumerate() {
                            if ui.button(entry.to_str().unwrap_or_default()).clicked() {
                                let to_load = self.cfg.recent_roms.remove(i).unwrap();
                                self.load_rom_from_file(to_load, false);
                                break;
                            }
                        }

                        if ui.button("Clear").clicked() {
                            self.cfg.recent_roms.clear();
                        }
                    });

                    #[cfg(all(not(target_arch = "wasm32"), feature = "savestates"))]
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
                                // TODO: this requires check if the current game is the same as the savestate
                                // if ui.button("To file...").clicked() {
                                //     // TODO: show file modal to save state
                                // }

                                // ui.separator();
                                for i in 1..9 {
                                    if ui.button(format!("Slot {i}")).clicked() {
                                        self.save_state(&i.to_string());
                                    }
                                }
                            });

                            ui.menu_button("Load Slots...", |ui| {
                                // TODO: this requires check if the current game is the same as the savestate
                                // if ui.button("From file...").clicked() {
                                //     // TODO: show file modal to load state
                                // }

                                // ui.separator();
                                for i in 1..9 {
                                    if ui.button(format!("Slot {i}")).clicked() {
                                        self.load_state(&i.to_string());
                                    }
                                }
                            });

                            #[cfg(target_os = "windows")]
                            if ui.button("Open states directory").clicked() {
                                use std::process;

                                match process::Command::new("explorer.exe")
                                    .arg(self.get_user_dir())
                                    .spawn()
                                {
                                    Err(e) => self
                                        .add_message(format!("couldn't open file explorer: {e}")),
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
                                let dir = self.get_user_dir();
                                _ = std::fs::remove_dir_all(dir);
                            }
                        })
                    });

                    // if ui.button("📷 Screenshot").clicked() {
                    //     // TODO: dump texture to file
                    //     eprintln!("screenshots not yet implemented");
                    // }

                    #[cfg(not(target_arch = "wasm32"))]
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
                                self.resume_emulation();
                            } else if reset.clicked() {
                                self.reset_emulation();
                            } else if stop.clicked() {
                                self.stop_emulation();
                            }
                        }

                        EmulationState::Running => {
                            let pause = ui.button("⏸ Pause");
                            ui.separator();
                            let reset = ui.button("🔄 Reset");
                            let stop = ui.button("⏹ Stop");

                            if pause.clicked() {
                                self.pause_emulation();
                            } else if reset.clicked() {
                                self.reset_emulation();
                            } else if stop.clicked() {
                                self.stop_emulation();
                            }
                        }
                    }

                    {
                        let mut emu = self.emu_lock();
                        let header = emu.rom_info();
                        if header.format == rom::HeaderFormat::Fds {
                            ui.separator();
                            if ui.button("💿 Insert next FDS disk/side").clicked() {
                                emu.mapper.special_input();
                            }
                        }
                    }
                });

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

                    if ui
                        .add_enabled(self.state.current_rom.is_some(), rom_info)
                        .clicked()
                    {
                        self.state.rom_info_open = true;
                    }

                    if ui.button("👢 Run FDS BIOS").clicked() {
                        match &self.state.bios {
                            Some(bios) => {
                                let new_emu = NesEmulator::bios_only(bios);
                                // this shouldnt fail but you never know
                                match new_emu {
                                    Ok(new_emu) => {
                                        self.close_and_save_rom_if_open();
                                        self.state.current_rom_header = new_emu.rom_info().clone();
                                        *self.emu_lock() = new_emu;

                                        self.resume_emulation();
                                    }
                                    Err(e) => self.add_message(e),
                                }
                            }
                            None => self.add_message("no BIOS ROM provided"),
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

                    let volume_slider = egui::Slider::new(&mut self.cfg.volume, 0.0..=2.0);
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
                ui.checkbox(&mut self.cfg.hide_exit_dialog, "Don't show again");

                ui.separator();
                ui.horizontal_centered(|ui| {
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
        let audio_disabled = self.cfg.disable_audio;
        let mut settings_open = self.state.settings_open;

        egui::Window::new("🔧 Settings")
            .collapsible(true)
            .resizable([true, true])
            .open(&mut settings_open)
            .show(ui, |ui| egui::ScrollArea::vertical().show(ui, |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("🎨 Load palette file...").clicked() {
                    self.file_dialog.open_dialog(FileOpenKind::NesPalette, "Select a NES palette file", "NES PAL file", &["pal"]);
                }

                ui.separator();

                let settings = &mut self.cfg.nes_settings;

                ui.collapsing(" Misc", |ui| {
                    ui.checkbox(&mut self.cfg.battery_save_enabled, "Enable battery saving")
                    .on_hover_text("This will dump work RAM in the same directory as the ROM's.");

                    #[cfg(all(not(target_arch = "wasm32"), feature = "savestates"))]
                    ui.checkbox(&mut self.cfg.restore_session, "Automatically restore last session when a game is reopened later");

                    ui.checkbox(&mut settings.random_ram, "Enable randomized RAM at startup")
                    .on_hover_text("Some games (such as Final Fantasy) use the random state of RAM at boot to seed their rngs");

                    ui.checkbox(&mut self.cfg.hide_exit_dialog, "Show exit dialog");
                });

                ui.separator();

                ui.collapsing("📺 Video", |ui| {
                    ui.checkbox(&mut settings.disable_sprite_limit, "Show more than 8 sprites per scaline")
                    .on_hover_text("Reduces flickering, but may show glitches in some games");
                    ui.checkbox(&mut settings.enable_accurate_ppu, "Enable fully emulated OAM read and VRAM read")
                    .on_hover_text("Fully emulates OAM and VRAM read with its quirks, might decrease performance");
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

                ui.separator();

                ui.collapsing("🔊 Audio", |ui| {
                    ui.checkbox(&mut self.cfg.disable_audio, "Disable audio and drive emulation by video")
                    .on_hover_text("By driving emulation with video, we get better frame pacing and no skipped frames");


                    ui.add_enabled_ui(self.audio.is_enabled(), |ui| {
                        if let Some(curr_device) = self.audio.current_device().cloned() {
                            ui.label("Audio device");
                            ui.indent("Audio devices", |ui| {
                                let mut selected_device = curr_device.clone();

                                if let Some(default) = self.audio.host.default_output_device() {
                                    ui.radio_value(&mut selected_device, default, "Default audio device");
                                }

                                if let Ok(devices) = self.audio.host.output_devices() {
                                    for dev in devices.into_iter() {
                                        let descr = dev.description().unwrap();
                                        let name = descr.name();
                                        ui.radio_value(&mut selected_device, dev, name);
                                    }

                                    if curr_device != selected_device {
                                        self.audio.set_ouput_device(selected_device, &self.emu);
                                        self.emu_lock().set_audio_rate(self.audio.sample_rate() as f64);
                                    }
                                }
                            });
                        }
                    });

                    // ui.label("Audio sample rate:");
                    // ui.indent("Sample rates", |ui| {
                    //     ui.radio_value(&mut self.cfg.sample_rate, 32000, "32000hz");
                    //     ui.radio_value(&mut self.cfg.sample_rate, 44100, "44100hz");
                    //     ui.radio_value(&mut self.cfg.sample_rate, 48000, "48000hz");
                    //     ui.radio_value(&mut self.cfg.sample_rate, 96000, "96000hz");
                    // });

                    // ui.label("Audio latency in milliseconds");
                    // let drag = egui::DragValue::new(&mut self.cfg.latency)
                    //     .range(1..=100)
                    //     .clamp_existing_to_range(true)
                    //     .update_while_editing(false);
                    // ui.add(drag);

                    let settings = &mut self.cfg.nes_settings;

                    ui.checkbox(&mut settings.enable_pulse0, "Enable pulse 0 channel");
                    ui.checkbox(&mut settings.enable_pulse1, "Enable pulse 1 channel");
                    ui.checkbox(&mut settings.enable_triangle, "Enable triangle channel");
                    ui.checkbox(&mut settings.enable_noise, "Enable noise channel");
                    ui.checkbox(&mut settings.enable_dmc, "Enable dmc channel");
                    ui.checkbox(&mut settings.enable_ext_audio, "Enable external sound chip");
                });

                ui.separator();

                #[cfg(not(target_arch = "wasm32"))]
                ui.collapsing("💿 Famicon Disk System (FDS)", |ui| {
                    if ui.button("👢 Load FDS BIOS file...").clicked() {
                        self.file_dialog.open_dialog(FileOpenKind::FdsBios, "Select FDS BIOS file", "FDS BIOS", &["rom"]);
                    }

                    if let Some(path) = &self.cfg.bios_path {
                        ui.separator();
                        ui.label(format!("👢 BIOS selected at: {:?}", path));
                    }
                })
            }));

        self.state.settings_open = settings_open;

        if audio_disabled != self.cfg.disable_audio {
            self.audio.set_enabled(!self.cfg.disable_audio);
            self.emu_lock().get_audiobuf().clear();
        }

        {
            let mut emu = self.emu_lock();
            if self.cfg.nes_settings != emu.settings {
                emu.set_settings(self.cfg.nes_settings.clone());
            }
        }
    }

    fn show_keybids_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("🎮 Keybindings")
            .collapsible(true)
            .resizable([false, false])
            .open(&mut self.state.keybinds_open)
            .show(ui, |ui| {
                ui.label("Left click to remove binding");

                // update bindings
                ui.input(|i| {
                    let binds = &mut self.cfg.input;

                    if i.pointer.secondary_clicked() {
                        binds.rebind = None;
                    }

                    if let Some(rebind_name) = &binds.rebind {
                        let bind = binds
                            .binds
                            .iter_mut()
                            .find(|x| x.name.eq(rebind_name))
                            .unwrap();

                        // take the first key pressed
                        if let Some(new_key) = i.keys_down.iter().next() {
                            if binds.keys_taken.contains(new_key) {
                                return Some(&bind.name);
                            }

                            bind.keys.push(*new_key);
                            binds.keys_taken.insert(*new_key);
                            binds.rebind = None;
                        }

                        if let Some(api) = &mut self.gamepads.api {
                            while let Some(evt) = api.next_event() {
                                match evt.event {
                                    gilrs::EventType::ButtonPressed(new_btn, _) => {
                                        if binds.btns_taken.contains(&new_btn) {
                                            return Some(&bind.name);
                                        }

                                        bind.btns.push(new_btn);
                                        binds.btns_taken.insert(new_btn);
                                        binds.rebind = None;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    return None;
                });

                let input = &mut self.cfg.input;

                let mut to_add = None;
                let mut to_remove = None;
                enum BindKind {
                    Key(usize),
                    Btn(usize),
                }

                for bind in &input.binds {
                    ui.separator();
                    ui.columns_const::<2, _>(|ui| {
                        let col_src = ui[0].vertical(|ui| {
                            ui.label(&bind.name);
                            if ui.button("Add bind").clicked() {
                                to_add = Some(&bind.name)
                            }
                        });

                        ui[1].vertical(|ui| {
                            for (idx, key) in bind.keys.iter().enumerate() {
                                let button = ui.button(format!("{:?}", key));
                                if button.clicked() {
                                    to_add = Some(&bind.name);
                                } else if button.secondary_clicked() {
                                    to_remove = Some((&bind.name, BindKind::Key(idx)))
                                }
                            }

                            if bind.btns.len() > 0 {
                                ui.separator();
                            }

                            for (idx, btn) in bind.btns.iter().enumerate() {
                                let button = ui.button(format!("{:?}", btn));
                                if button.clicked() {
                                    to_add = Some(&bind.name);
                                } else if button.secondary_clicked() {
                                    to_remove = Some((&bind.name, BindKind::Btn(idx)))
                                }
                            }
                        });

                        if let Some(rebind_name) = &to_add {
                            if bind.name.eq(*rebind_name) {
                                col_src.response.highlight();
                            }
                        }
                    });
                }

                if let Some(selected) = to_add {
                    input.rebind = Some(selected.clone());
                }

                if let Some((selected_name, id)) = to_remove {
                    let bind_idx = input
                        .binds
                        .iter()
                        .position(|x| x.name.eq(selected_name))
                        .unwrap();

                    match id {
                        BindKind::Key(idx) => {
                            let key = input.binds[bind_idx].keys.swap_remove(idx);
                            input.keys_taken.remove(&key);
                        }
                        BindKind::Btn(idx) => {
                            let btn = input.binds[bind_idx].btns.swap_remove(idx);
                            input.btns_taken.remove(&btn);
                        }
                    }
                }

                ui.vertical_centered(|ui| {
                    if let Some(rebind_name) = &input.rebind {
                        ui.label(format!(
                            "Rebinding {:?}... Press any button, right click or close window to cancel",
                            rebind_name
                        ));
                    }
                });
            })
            .or_else(|| {
                self.cfg.input.rebind = None;
                None
            });
    }

    fn show_rom_info_window(&mut self, ui: &mut egui::Ui) {
        let header = &self.state.current_rom_header;

        egui::Window::new("💾 ROM information")
            .collapsible(true)
            .open(&mut self.state.rom_info_open)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.columns(2, |ui| {
                        ui[0].label("Game Title");
                        ui[1].label(&header.title);
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("Header kind");
                        ui[1].label(header.format.to_string());
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("Mapper ID");
                        ui[1].hyperlink_to(
                            format!("{} ({})", header.mapper_name, header.mapper),
                            format!(
                                "https://www.nesdev.org/wiki/INES_Mapper_{:03}",
                                header.mapper
                            ),
                        );
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("SubMapper ID");
                        ui[1].label(header.submapper.to_string());
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("Region");
                        ui[1].label(format!("{:?}", header.region));
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("Mirroring");
                        let mirroring = if header.alt_mirroring {
                            "Alternative"
                        } else {
                            &format!("{:?}", header.mirroring)
                        };
                        ui[1].label(mirroring);
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("PRG size");
                        ui[1].add(
                            egui::ProgressBar::new((header.prg_size / 1024) as f32 / 512.0)
                                .corner_radius(egui::CornerRadius::ZERO)
                                .text(format!("{} KiB", header.prg_size / 1024)),
                        )
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("WRAM size");
                        ui[1].add(
                            egui::ProgressBar::new((header.wram_size / 1024) as f32 / 32.0)
                                .corner_radius(egui::CornerRadius::ZERO)
                                .text(format!("{} KiB", header.wram_size / 1024)),
                        )
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("CHR size");
                        ui[1].add(
                            egui::ProgressBar::new((header.chr_size / 1024) as f32 / 256.0)
                                .corner_radius(egui::CornerRadius::ZERO)
                                .text(format!("{} KiB", header.chr_size / 1024)),
                        )
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("CHR RAM");
                        ui[1].label(if header.has_chr_ram { "☑" } else { "☐" });
                    });

                    ui.columns(2, |ui| {
                        ui[0].label("Battery");
                        ui[1].label(if header.has_battery { "☑" } else { "☐" })
                    });
                })
            });
    }

    fn show_about_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("ℹ About")
            .collapsible(true)
            .resizable(true)
            .open(&mut self.state.about_open)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.hyperlink_to("Nen Emulator ", "https://github.com/Comba92/nen-emulator");
                    ui.label("Developed by:");
                    ui.hyperlink_to("Comba92 ", "https://github.com/Comba92");

                    ui.hyperlink_to(
                        "Report bugs or issues",
                        "https://github.com/Comba92/nen-emulator/issues/new/choose",
                    );
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

            // TODO: time doesn't work on wasm32, should use browser API time
            #[cfg(not(target_arch = "wasm32"))]
            {
                const MSG_DELAY: time::Duration = time::Duration::from_secs(4);
                if appeared.elapsed() > MSG_DELAY {
                    *open = false;
                }
            }

            if let None = res {
                self.state.message_open = None;
            }
        }
    }

    fn handle_file_dialog(&mut self) {
        match self.file_dialog.recv.try_recv() {
            Ok((bytes, path, kind)) => match kind {
                FileOpenKind::NesRom => {
                    self.load_rom_from_bytes(path, bytes.into_boxed_slice(), false)
                }
                FileOpenKind::NesPalette => self.load_palette_from_bytes(&bytes),
                FileOpenKind::FdsBios => {
                    if is_valid_bios(&bytes) {
                        self.state.bios = Some(bytes.into_boxed_slice());
                        self.cfg.bios_path = Some(path);
                    } else {
                        self.cfg.bios_path = None;
                        self.add_message("not a valid FDS bios");
                    }
                }
            },

            _ => {}
        }
    }

    fn handle_action(&mut self, act: EmulatorAction) {
        match act {
            EmulatorAction::Reset => self.reset_emulation(),
            EmulatorAction::TogglePause => match self.state.emulation {
                EmulationState::Paused => self.resume_emulation(),
                EmulationState::Running => self.pause_emulation(),
                EmulationState::Stopped => {}
            },
            EmulatorAction::ToggleMute => {
                self.audio.muted = !self.audio.muted;
                if self.audio.muted {
                    *self.audio.volume.lock().unwrap() = 0.0;
                } else {
                    *self.audio.volume.lock().unwrap() = self.cfg.volume;
                }
            }

            #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
            _ => {}
        }
    }

    fn handle_input_and_emulation(&mut self, ui: &mut egui::Ui) {
        let keyboard_input = ui.input(|i| {
            let mut pressed = joypad::JoypadInput::empty();

            'poll_loop: for bind in &self.cfg.input.binds {
                for key in &bind.keys {
                    match &bind.kind {
                        PlayerInput::Joypad(emu_btn) => {
                            if i.key_down(*key) {
                                pressed.insert(*emu_btn);
                            }
                        }
                        PlayerInput::Action(act) => {
                            if i.key_pressed(*key) {
                                self.handle_action(*act);
                                break 'poll_loop;
                            }
                        }
                    }
                }
            }

            pressed
        });

        let mut gamepad_input = joypad::JoypadInput::empty();
        if let Some(api) = &mut self.gamepads.api {
            // pump events
            // CAUTION: next_event() is needed even if we dont want to handle any events, as it will cache them for is_pressed()
            while let Some(_) = api.next_event() {}

            'poll_loop: for (_, pad) in api.gamepads() {
                let x = pad.value(gilrs::Axis::LeftStickX);
                let y = pad.value(gilrs::Axis::LeftStickY);

                if x >= 0.2 {
                    gamepad_input.insert(joypad::JoypadInput::Right);
                } else if x <= -0.2 {
                    gamepad_input.insert(joypad::JoypadInput::Left);
                }

                if y >= 0.2 {
                    gamepad_input.insert(joypad::JoypadInput::Up);
                } else if y <= -0.2 {
                    gamepad_input.insert(joypad::JoypadInput::Down);
                }

                for bind in &self.cfg.input.binds {
                    for btn in &bind.btns {
                        if let Some(state) = pad.button_data(*btn) {
                            match bind.kind {
                                PlayerInput::Joypad(joy) => {
                                    if state.is_pressed() {
                                        gamepad_input.insert(joy);
                                    }
                                }
                                PlayerInput::Action(act) => {
                                    if state.is_pressed() && !state.is_repeating() {
                                        self.handle_action(act);
                                        break 'poll_loop;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let (mouse_left, mouse_right) = ui.input(|i| {
            if matches!(self.state.mouse_pos.0, 0..256) && matches!(self.state.mouse_pos.1, 0..240)
            {
                (
                    i.pointer.button_down(egui::PointerButton::Primary),
                    i.pointer.button_down(egui::PointerButton::Secondary),
                )
            } else {
                (false, false)
            }
        });

        if self.state.emulation == EmulationState::Running {
            {
                let mut emu = self.emu_lock();

                if keyboard_input != self.state.keyboard_input {
                    emu.set_buttons_all(keyboard_input);
                } else if gamepad_input != self.state.gamepad_input {
                    emu.set_buttons_all(gamepad_input);
                }

                emu.set_zapper_trigger(mouse_left);
                emu.set_zapper_light_outside(mouse_right);
                emu.set_zapper_light(self.state.mouse_pos.0, self.state.mouse_pos.1);

                match emu.step_until_frame_ready() {
                    Ok(_) => {
                        let framebuf = egui::ColorImage::from_rgba_unmultiplied(
                            [256, 240],
                            emu.get_video_rgba(),
                        );
                        drop(emu);
                        self.tex.set(framebuf, TEX_OPTS);
                    }

                    Err(e) => {
                        drop(emu);
                        self.stop_emulation();
                        self.add_message(e);
                    }
                }
            }

            self.state.keyboard_input = keyboard_input;
            self.state.gamepad_input = gamepad_input;
        } else if self.state.emulation == EmulationState::Stopped {
            egui::Window::new("Start any ROM")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .title_bar(false)
                .collapsible(false)
                .interactable(false)
                .auto_sized()
                .show(ui, |ui| {
                    ui.heading("Open a ROM to start.");
                });
        }
    }
}

impl eframe::App for AppCtx {
    #[cfg(all(not(target_arch = "wasm32"), feature = "persistence"))]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.cfg);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menubar")
            .show_separator_line(true)
            .show_inside(ui, |ui| self.show_menubar(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                egui::Frame::new()
                    .fill(egui::Color32::BLACK.gamma_multiply(0.6))
                    .show(ui, |ui| {
                        let img = egui::Image::new(&self.tex)
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

        #[cfg(not(target_arch = "wasm32"))]
        ui.input(|i| {
            // check for dropped files
            let files = &i.raw.dropped_files;
            if let Some(Some(path)) = files.first().map(|f| &f.path) {
                let pal_ext = std::ffi::OsStr::new("pal");
                if path.extension() == Some(pal_ext) {
                    self.load_palette_from_file(path);
                } else {
                    self.load_rom_from_file(path, true);
                }
            }
        });

        if ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::ALT, egui::Key::Enter) {
                self.cfg.fullscreen = !self.cfg.fullscreen;
                true
            } else {
                false
            }
        }) {
            ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.cfg.fullscreen));
        }

        self.show_settings_window(ui);
        self.show_keybids_window(ui);
        self.show_about_window(ui);
        #[cfg(not(target_arch = "wasm32"))]
        self.show_exit_dialog(ui);
        self.show_rom_info_window(ui);
        self.show_error_window(ui);

        self.handle_file_dialog();
        self.handle_input_and_emulation(ui);

        if !self.audio.muted {
            *self.audio.volume.lock().unwrap() = self.cfg.volume;
        }

        ui.request_repaint();
        self.fps.delay();

        #[cfg(not(target_arch = "wasm32"))]
        if !self.cfg.hide_exit_dialog {
            if ui.input(|i| i.viewport().close_requested()) {
                if !self.state.should_close {
                    ui.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.state.exit_modal_open = true;
                }
            }
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "savestates"))]
impl AppCtx {
    fn get_user_dir(&self) -> PathBuf {
        // todo: this fails on mobile lol
        let mut dir = eframe::storage_dir(APP_NAME).unwrap();
        #[cfg(target_os = "windows")]
        dir.pop(); // on linux/mac this goes up to the root of the shared folder
        dir.push("states");
        dir
    }

    fn get_rom_states_dir(&self) -> PathBuf {
        let mut dir = self.get_user_dir();
        // todo: too many unwraps... scary
        let (_, current_rom_path) = self.state.current_rom.as_ref().unwrap();
        dir.push(current_rom_path.file_stem().unwrap());
        dir
    }

    fn save_state(&mut self, name: &str) {
        let mut dir = self.get_user_dir();

        // todo: too many unwraps... scary
        let (_, current_rom_path) = self.state.current_rom.as_ref().unwrap();
        dir.push(current_rom_path.file_stem().unwrap());

        _ = std::fs::create_dir_all(&dir);

        dir.push(name);
        dir.set_extension("state");
        let res = self.emu_lock().savestate(dir);
        if let Err(e) = res {
            self.add_message(e);
        }
    }

    fn load_state(&mut self, name: &str) {
        let mut dir = self.get_rom_states_dir();

        dir.push(name);
        dir.set_extension("state");
        _ = self.emu_lock().loadstate(dir);
    }
}
