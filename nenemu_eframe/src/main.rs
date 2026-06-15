use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use nenemu_core::{NesPalette, emu::NesEmulator, joypad, rom, utils::RingBuffer};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, atomic},
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
    Joypad(joypad::JoypadBtn),
    Action(EmulatorAction),
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct KeyMap {
    keys: HashMap<egui::Key, joypad::JoypadBtn>,

    pads: HashMap<gilrs::Button, joypad::JoypadBtn>,
    rebind_key: Option<(egui::Key, joypad::JoypadBtn)>,
}

impl Default for KeyMap {
    fn default() -> Self {
        use egui::Key;
        use joypad::JoypadBtn as Btn;

        let keys = HashMap::from([
            (Key::ArrowUp, Btn::Up),
            (Key::ArrowDown, Btn::Down),
            (Key::ArrowLeft, Btn::Left),
            (Key::ArrowRight, Btn::Right),
            (Key::A, Btn::B),
            (Key::S, Btn::A),
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
                (Button::West, Btn::B),
                (Button::South, Btn::A),
                (Button::Start, Btn::Start),
                (Button::Select, Btn::Select),
            ])
        };

        Self {
            keys,

            pads,
            rebind_key: None,
        }
    }
}

struct GamepadHandler {
    pub api: gilrs::Gilrs,
    pub active: Option<gilrs::GamepadId>,
}

impl Default for GamepadHandler {
    fn default() -> Self {
        let api = gilrs::Gilrs::new().unwrap();
        let active = api.gamepads().next().map(|x| x.0);
        Self { api, active }
    }
}

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

struct AudioHandler {
    stream: Option<cpal::Stream>,
}

impl AudioHandler {
    pub fn new(sample_rate: u32, buf_size: u32, emu: Arc<Mutex<NesEmulator>>) -> Self {
        let host = cpal::default_host();
        match host.default_output_device() {
            Some(device) => {
                let config = cpal::StreamConfig {
                    channels: 2,
                    sample_rate: sample_rate,
                    buffer_size: cpal::BufferSize::Fixed(buf_size),
                };

                let stream = device
                    .build_output_stream(
                        config,
                        move |audio_out, _| {
                            let mut emu_lock = emu.lock().unwrap();

                            let (right, left) = emu_lock.get_audio_f32(audio_out.len() / 2);
                            for i in 0..right.len() {
                                audio_out[2 * i] = right[i];
                                audio_out[2 * i + 1] = right[i];
                            }

                            if let Some(left) = left {
                                let audio_out = &mut audio_out[2 * right.len()..];
                                for i in 0..left.len() {
                                    audio_out[2 * i] = left[i];
                                    audio_out[2 * i + 1] = left[i];
                                }
                            }
                        },
                        |err| eprintln!("{err}"),
                        None,
                    )
                    .unwrap();

                stream.play().unwrap();

                Self {
                    stream: Some(stream),
                }
            }

            None => Self { stream: None },
        }
    }

    pub fn is_supported(&self) -> bool {
        self.stream.is_some()
    }

    pub fn buffer_size(&self) -> Option<u32> {
        self.stream
            .as_ref()
            .map(|s| s.buffer_size().unwrap_or_default())
    }

    pub fn resume(&self) {
        match &self.stream {
            Some(stream) => stream.play().unwrap(),
            _ => {}
        }
    }

    pub fn pause(&self) {
        match &self.stream {
            Some(stream) => stream.pause().unwrap(),
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

fn emulation_thread_proc(
    emu: Arc<Mutex<NesEmulator>>,
    video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
    samples_needed: usize,
    is_running: Arc<atomic::AtomicBool>,
) {
    let frame_rate = time::Duration::from_secs_f32(1.0 / 288.0);
    loop {
        let frame_start = time::Instant::now();

        if is_running.load(atomic::Ordering::Relaxed) {
            let mut emu_lock = emu.lock().unwrap();
            while emu_lock.audio_queued() < samples_needed {
                emu_lock
                    .step_until_samples_or_frame_ready(samples_needed)
                    .unwrap();

                if emu_lock.is_frame_ready() {
                    let framebuf = egui::ColorImage::from_rgba_unmultiplied(
                        [256, 240],
                        emu_lock.get_video_rgba(),
                    );
                    video_chain.lock().unwrap().push(framebuf);
                }
            }
        }

        sleep_until_fps(frame_start, frame_rate);
    }
}

fn emulation_thread_no_audio_proc(
    emu: Arc<Mutex<NesEmulator>>,
    video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
    is_running: Arc<atomic::AtomicBool>,
) {
    let frame_rate = time::Duration::from_secs_f32(1.0 / 61.0);
    loop {
        let frame_start = time::Instant::now();

        if is_running.load(atomic::Ordering::Relaxed) {
            let mut emu_lock = emu.lock().unwrap();
            emu_lock.step_until_frame_ready().unwrap();

            let framebuf =
                egui::ColorImage::from_rgba_unmultiplied([256, 240], emu_lock.get_video_rgba());
            video_chain.lock().unwrap().push(framebuf);
        }

        sleep_until_fps(frame_start, frame_rate);
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
enum EmulationState {
    #[default]
    Stopped,
    Running,
    Paused,
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

    restore_session: bool,

    nes_settings: nenemu_core::emu::Settings,

    volume: f32,
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

    current_rom_path: Option<PathBuf>,
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
    emu_thread: thread::JoinHandle<()>,
    is_running: Arc<atomic::AtomicBool>,

    video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
    tex: Arc<Mutex<egui::TextureHandle>>,

    audio: AudioHandler,

    gamepads: GamepadHandler,

    state: AppState,
    cfg: AppCfg,
}

impl AppCtx {
    pub fn new(c: &eframe::CreationContext) -> Box<Self> {
        #[cfg(not(feature = "persistence"))]
        let cfg = AppCfg::default();

        #[cfg(feature = "persistence")]
        let cfg = if let Some(storage) = c.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            AppCfg::default()
        };

        let img = egui::ColorImage::filled([256, 240], egui::Color32::TRANSPARENT);
        let tex = c.egui_ctx.load_texture("emu_present", img, TEX_OPTS);
        let tex = Arc::new(Mutex::new(tex));

        let emu = NesEmulator::empty();
        let emu = Arc::new(Mutex::new(emu));
        // let sdl = SdlCtx::new(44100, Arc::clone(&emu));

        let video_chain = Arc::new(Mutex::new(RingBuffer::new(8)));

        let audio = AudioHandler::new(48000, 512, Arc::clone(&emu));
        let samples_needed = audio.buffer_size().unwrap_or(1024);

        let emu_arc = Arc::clone(&emu);
        let chain_arc = Arc::clone(&video_chain);

        let is_running = Arc::new(atomic::AtomicBool::new(false));
        let is_running_arc = Arc::clone(&is_running);

        let emu_thread = if audio.is_supported() {
            thread::Builder::new()
                .name("emulation".into())
                .spawn(move || {
                    emulation_thread_proc(
                        emu_arc,
                        chain_arc,
                        samples_needed as usize,
                        is_running_arc,
                    )
                })
                .unwrap()
        } else {
            thread::Builder::new()
                .name("emulation".into())
                .spawn(move || emulation_thread_no_audio_proc(emu_arc, chain_arc, is_running_arc))
                .unwrap()
        };

        let res = Self {
            // sdl,
            emu,
            emu_thread,
            is_running,
            video_chain,
            tex,
            audio,

            gamepads: GamepadHandler::default(),

            cfg,
            state: Default::default(),
        };

        Box::new(res)
    }

    fn emu_lock(&self) -> MutexGuard<'_, NesEmulator> {
        self.emu.lock().unwrap()
    }

    fn add_message(&mut self, e: GenericError) {
        self.state.message_open = Some((true, time::Instant::now(), e));
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

    fn load_rom<P: AsRef<Path>>(&mut self, rom_path: P) {
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

                new_emu.settings = self.cfg.nes_settings.clone();
                if let Some(pal) = self.cfg.palettes.front() {
                    new_emu.palette = pal.clone();
                }

                self.state.current_rom_header = {
                    let mut emu = self.emu_lock();
                    *emu = new_emu;
                    emu.header().clone()
                };

                let pathbuf = rom_path.as_ref().to_path_buf();
                self.state.current_rom_path = Some(pathbuf.clone());
                ring_push_front(&mut self.cfg.recent_roms, pathbuf, 12);

                #[cfg(feature = "savestates")]
                if self.cfg.restore_session {
                    self.load_state("last");
                }

                self.state.emulation = match self.state.emulation {
                    EmulationState::Stopped | EmulationState::Running => {
                        // self.audio_stream.play().unwrap();
                        EmulationState::Running
                    }
                    EmulationState::Paused => {
                        // self.audio_stream.pause().unwrap();
                        EmulationState::Paused
                    }
                };
            }

            Err(e) => {
                self.add_message(e);
            }
        }
    }

    fn show_menubar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |content| {
            content.horizontal_wrapped(|ui| {
                ui.menu_button("💾 File", |ui| {
                    if ui.button("📂 Open...").clicked() {
                        file_dialog("Select game ROM", "NES ROM", &["nes", "fds", "zip", "rar"])
                            .map(|path| self.load_rom(path));
                    }

                    ui.menu_button("Recent ROMs", |ui| {
                        if self.cfg.recent_roms.is_empty() {
                            ui.label("No recent ROMs");
                            return;
                        }

                        for (i, entry) in self.cfg.recent_roms.iter().enumerate() {
                            if ui.button(entry.to_str().unwrap_or_default()).clicked() {
                                let to_load = self.cfg.recent_roms.remove(i).unwrap();
                                self.load_rom(to_load);
                                break;
                            }
                        }

                        if ui.button("Clear").clicked() {
                            self.cfg.recent_roms.clear();
                        }
                    });

                    let running = self.state.emulation == EmulationState::Running;

                    #[cfg(feature = "savestates")]
                    ui.add_enabled_ui(running, |ui| {
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

                            if ui.button("Copy states directory to clipboard").clicked() {
                                ui.copy_text(self.get_states_dir().to_string_lossy().into_owned());
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
                                self.audio.resume();
                            } else if reset.clicked() {
                                eprintln!("reset not implemented");
                            } else if stop.clicked() {
                                self.state.emulation = EmulationState::Stopped;
                                self.audio.pause();
                            }
                        }

                        EmulationState::Running => {
                            let pause = ui.button("⏸ Pause");
                            ui.separator();
                            let reset = ui.button("🔄 Reset");
                            let stop = ui.button("⏹ Stop");

                            if pause.clicked() {
                                self.state.emulation = EmulationState::Paused;
                                self.audio.pause();
                            } else if reset.clicked() {
                                eprintln!("reset not implemented");
                            } else if stop.clicked() {
                                self.state.emulation = EmulationState::Stopped;
                                self.audio.pause();
                            }
                        }
                    }

                    let mut emu = self.emu_lock();
                    let header = emu.header();
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

                if running {
                    let style = ui.style_mut();
                    style.spacing.slider_width *= 0.7;

                    ui.separator();
                    ui.label("🔊 Vol");

                    let volume_slider = egui::Slider::new(&mut self.cfg.volume, 0.0..=1.0);
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
        let were_settings_open = self.state.settings_open;
        let mut should_update_palette = None;

        egui::Window::new("🔧 Settings")
            .collapsible(true)
            .resizable([true, true])
            .open(&mut self.state.settings_open)
            .show(ui, |ui| {
                let settings = &mut self.cfg.nes_settings;

                if ui.button("🎨 Load palette file...").clicked() {
                    should_update_palette = file_dialog("Select a NES palette file", "NES PAL file", &["pal"]);
                }

                ui.collapsing(" Misc", |ui| {
                    ui.checkbox(&mut self.cfg.battery_save_enabled, "Enable battery saving")
                    .on_hover_text("This will dump work RAM in the same directory as the ROM's.");

                    #[cfg(feature = "savestates")]
                    ui.checkbox(&mut self.cfg.restore_session, "Automatically restore last session when a game is reopened later");

                    ui.checkbox(&mut settings.random_ram, "Randomize RAM at startup")
                    .on_hover_text("Some games (such as Final Fantasy) use the random state of RAM at boot to seed their rngs");
                });

                ui.collapsing("📺 Video", |ui| {
                    ui.checkbox(&mut settings.no_sprite_limit, "Show more than 8 sprites per scaline")
                    .on_hover_text("Reduces flickering, but may show glitches in some games");
                    ui.checkbox(&mut settings.disable_background, "Disable background tiles");
                    ui.checkbox(&mut settings.disable_sprites, "Disable sprite tiles");
                    ui.checkbox(&mut settings.pal_borders, "Show side PAL black borders");
                });
                ui.collapsing("🔊 Audio", |ui| {
                    ui.label("Audio sample rate:");
                    ui.indent("Sample rates", |ui| {
                    ui.radio_value(&mut settings.audio_sample_rate, 32000, "32000hz");
                    ui.radio_value(&mut settings.audio_sample_rate, 44100, "44100hz");
                    ui.radio_value(&mut settings.audio_sample_rate, 48000, "48000hz");
                    ui.radio_value(&mut settings.audio_sample_rate, 96000, "96000hz");
                });

                    ui.checkbox(&mut settings.disable_pulse0, "Disable pulse 0 channel");
                    ui.checkbox(&mut settings.disable_pulse1, "Disable pulse 1 channel");
                    ui.checkbox(&mut settings.disable_triangle, "Disable triangle channel");
                    ui.checkbox(&mut settings.disable_noise, "Disable noise channel");
                    ui.checkbox(&mut settings.disable_dmc, "Disable dmc channel");
                    ui.checkbox(&mut settings.disable_ext_audio, "Disable external sound chip");
                });

                ui.collapsing("💿 Famicon Disk System (FDS)", |ui| {
                    let bios_btn_text = if let Some(path) = &self.cfg.bios_path {
                    format!("👢 BIOS selected at: {:?}, click to change...", path)
                    } else {
                        "👢 Load FDS BIOS file...".to_string()
                    };

                    if ui.button(bios_btn_text).clicked() {
                        file_dialog("Select FDS BIOS file", "FDS BIOS", &["rom"])
                            .map(|path| self.cfg.bios_path = Some(path));
                    }

                    // TODO: disk handling
                })
            });

        {
            if self.state.settings_open != were_settings_open {
                self.emu_lock().settings = self.cfg.nes_settings.clone();
            }

            if let Some(pal_path) = should_update_palette {
                self.load_palette(pal_path);
            }
        }
    }

    fn show_keybids_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("🎮 Keybindings")
            .collapsible(true)
            .resizable([true, true])
            .open(&mut self.state.keybinds_open)
            .show(ui, |ui| {
                const BTN_NAMES: &[&str] =
                    &["Up", "Down", "Left", "Right", "A", "B", "Start", "Select"];

                for (key, btn_name) in self.cfg.keymaps.keys.iter().zip(BTN_NAMES.iter()) {
                    ui.columns_const::<2, _>(|ui| {
                        let col1 = ui[0].label(*btn_name);
                        let col2 = ui[1].button(format!("{:?}", key.0));

                        if let Some(rebind_key) = &self.cfg.keymaps.rebind_key {
                            if rebind_key.1 == *key.1 {
                                col1.highlight();
                                col2.highlight();
                            } else if col2.clicked() {
                                self.cfg.keymaps.rebind_key = Some((*key.0, *key.1));
                            }
                        } else if col2.clicked() {
                            self.cfg.keymaps.rebind_key = Some((*key.0, *key.1));
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

    fn handle_input(&mut self, ui: &mut egui::Ui) {
        let current_input = self.emu_lock().joypad.buttons.clone();

        let (keyboard_input, keyboard_changed) = ui.input(|i| {
            let mut pressed = current_input.clone();

            if !i.keys_down.is_empty() {
                for (key, emu_btn) in &self.cfg.keymaps.keys {
                    pressed.set(*emu_btn, i.key_down(*key));
                }
            }

            (pressed, !i.keys_down.is_empty())
        });

        let (mut gamepad_input, mut gamepad_changed) = (current_input.clone(), false);

        if let Some(active) = self.gamepads.active {
            while let Some(gilrs::Event { id, event, .. }) = self.gamepads.api.next_event() {
                if active != id {
                    continue;
                }

                gamepad_changed = true;
                match event {
                    gilrs::EventType::ButtonReleased(btn, _) => {
                        if let Some(emu_btn) = self.cfg.keymaps.pads.get(&btn) {
                            gamepad_input.remove(*emu_btn);
                        }
                    }

                    gilrs::EventType::ButtonPressed(btn, _) => {
                        if let Some(emu_btn) = self.cfg.keymaps.pads.get(&btn) {
                            gamepad_input.insert(*emu_btn);
                        }
                    }

                    gilrs::EventType::AxisChanged(axis, amt, _) => match axis {
                        gilrs::Axis::LeftStickX => {
                            if amt >= 0.1 {
                                gamepad_input.insert(joypad::JoypadBtn::Right);
                            } else if amt <= -0.1 {
                                gamepad_input.insert(joypad::JoypadBtn::Left);
                            } else {
                                gamepad_input.remove(joypad::JoypadBtn::Right);
                                gamepad_input.remove(joypad::JoypadBtn::Left);
                            }
                        }
                        gilrs::Axis::LeftStickY => {
                            if amt >= 0.1 {
                                gamepad_input.insert(joypad::JoypadBtn::Up);
                            } else if amt <= -0.1 {
                                gamepad_input.insert(joypad::JoypadBtn::Down);
                            } else {
                                gamepad_input.remove(joypad::JoypadBtn::Up);
                                gamepad_input.remove(joypad::JoypadBtn::Down);
                            }
                        }
                        _ => {}
                    },

                    _ => {}
                }
            }
        }

        {
            let mut emu = self.emu_lock();

            if keyboard_changed {
                emu.set_buttons_all(keyboard_input);
            }

            if gamepad_changed {
                emu.set_buttons_all(gamepad_input);
            }
        }
    }
}

impl eframe::App for AppCtx {
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.cfg);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let current_state = self.state.emulation;

        egui::Panel::top("menubar")
            .show_separator_line(true)
            .show_inside(ui, |ui| self.show_menubar(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                let tex = self.tex.lock().unwrap();
                let img = egui::Image::new(&*tex)
                    .maintain_aspect_ratio(self.cfg.keep_aspect_ratio)
                    .fit_to_exact_size(ui.max_rect().size());

                let screen = ui.add(img);
                if self.state.emulation == EmulationState::Running && self.cfg.hide_cursor {
                    screen.on_hover_cursor(egui::CursorIcon::None);
                }
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
                    self.load_rom(path);
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

        self.handle_input(ui);

        if self.state.emulation != current_state {
            self.is_running.store(
                self.state.emulation == EmulationState::Running,
                atomic::Ordering::Relaxed,
            );
        }

        {
            let mut video_lock = self.video_chain.lock().unwrap();
            if video_lock.queued() > 0 {
                let framebuf = std::mem::take(video_lock.pop_mut());
                self.tex.lock().unwrap().set(framebuf, TEX_OPTS);
            }
        }

        const FPS: f32 = 1.0 / 144.0;
        ui.request_repaint_after_secs(FPS);

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
