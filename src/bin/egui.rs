use std::{collections::VecDeque, mem};

use eframe::egui;
use nes_emulator::{emu, joypad};

const TEX_OPTS: egui::TextureOptions = egui::TextureOptions {
  magnification: egui::TextureFilter::Nearest,
  minification: egui::TextureFilter::Nearest,
  wrap_mode: egui::TextureWrapMode::ClampToEdge,
  mipmap_mode: None,
};

fn main() {
  let opts = eframe::NativeOptions {
    centered: true,
    viewport: egui::ViewportBuilder::default()
      .with_drag_and_drop(true)
      .with_inner_size((256.0 * 3.0, 240.0 * 3.0))
      .with_title("NenEmu"),
    vsync: true,
    hardware_acceleration: eframe::HardwareAcceleration::Preferred,

    ..Default::default()
  };

  eframe::run_native("NenEmu",
  opts,
  Box::new(
    |c| Ok(AppCtx::new(c))
  )).unwrap();
}

struct KeyMap {
  keys: Vec<(egui::Key, joypad::Button)>,
  rebind_key: Option<(egui::Key, joypad::Button)>,
}
impl Default for KeyMap {
  fn default() -> Self {
    use egui::Key;
    use joypad::Button as Btn;
    
    let keys = Vec::from([
      (Key::ArrowUp, Btn::Up),
      (Key::ArrowDown, Btn::Down),
      (Key::ArrowLeft, Btn::Left),
      (Key::ArrowRight, Btn::Right),
      (Key::Z, Btn::A),
      (Key::X, Btn::B),
      (Key::A, Btn::Start),
      (Key::S, Btn::Select),
    ]);

    Self { keys, rebind_key: None }
  }
}

#[derive(Default, PartialEq)]
enum EmuState {
  Running,
  Paused,
  Stopped,
  #[default] Off,
}

struct SdlCtx {
  _sdl: sdl2::Sdl,
  _audio: sdl2::AudioSubsystem,
  audiodev: sdl2::audio::AudioQueue<i16>,
}
impl Default for SdlCtx {
  fn default() -> Self {
    Self::new(48000)
  }
}

impl SdlCtx {
  pub fn new(sample_rate: usize) -> Self {
    let _sdl = sdl2::init().unwrap();
    let _audio = _sdl.audio().unwrap();
    let audiospec = sdl2::audio::AudioSpecDesired {
      channels: Some(1),
      freq: Some(sample_rate as i32),
      samples: None,
    };
    let audiodev = _audio.open_queue::<i16, _>(None, &audiospec).unwrap();
    audiodev.resume();

    Self {
      _sdl,
      _audio,
      audiodev,
    }
  }
}

#[derive(Default)]
struct AppWndCtx {
  keybinds_open: bool,
  rom_info_open: bool,
  about_open: bool,
}

struct AppCtx {
  emu: Option<(emu::Emu, EmuState)>,
  dt: f32,
  fps: f32,
  framebuf: egui::ColorImage,
  tex: egui::TextureHandle,

  video_size: i32,
  keep_aspect_ratio: bool,
  fullscreen: bool,
  settings: emu::Settings,

  windows: AppWndCtx,
  recents: VecDeque<std::path::PathBuf>,
  keymaps: KeyMap,
  sdl: SdlCtx,
}
impl AppCtx {
  pub fn new(c: &eframe::CreationContext) -> Box<Self> {
    let img = egui::ColorImage::filled([256, 240], egui::Color32::TRANSPARENT);
    let tex = c.egui_ctx.load_texture("tex", img.clone(), TEX_OPTS);
    let sdl = SdlCtx::default();

    Box::new(Self {
      emu: None,
      dt: 0.0,
      fps: 0.0,
      framebuf: img,
      tex,

      video_size: 3,
      keep_aspect_ratio: true,
      fullscreen: false,
      settings: emu::Settings::new(),

      keymaps: KeyMap::default(),
      windows: AppWndCtx::default(),
      recents: VecDeque::new(),
      sdl,
    })
  }

  fn load_rom<P: AsRef<std::path::Path>>(&mut self, path: P) {
    let res = emu::Emu::load_rom_from_file(&path);
    match res {
      Ok(new_emu) => {
        self.sdl.audiodev.clear();
        self.fps = 1.0 / new_emu.frame_rate();
        
        let new_state = if let Some((_, old_state)) = &mut self.emu {
          match old_state {
            EmuState::Off | EmuState::Stopped | EmuState::Running => EmuState::Running,
            EmuState::Paused => {
              self.sdl.audiodev.pause();
              EmuState::Paused
            }
          }
        } else {
          EmuState::Running
        };

        self.emu = Some((new_emu, new_state));
        
        self.recents.push_front(path.as_ref().to_path_buf());
        self.recents.truncate(12);
      }
      Err(e) => {
        // todo: show some kind of error
      }
    }
  }
}

impl eframe::App for AppCtx {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let running = self.emu.is_some();
    
    egui::TopBottomPanel::top("top")
    .show_separator_line(true)
    .show(ctx, |ui| {
      egui::MenuBar::new()
      .ui(ui, |content| {
        content.horizontal_wrapped(|ui| {
          ui.menu_button("💾 File", |ui| {
            if ui.button("📂 Open...").clicked() {
              let file = rfd::FileDialog::new()
              .set_can_create_directories(true)
              .set_title("Select game ROM")
              .add_filter("NES ROM", &["nes", "fds", "zip"])
              .pick_file();

              if let Some(path) = file {
                self.load_rom(path);
              } else {
                // TODO: show some kind of error
              }
            }

            ui.menu_button("Recents...", |ui| {
              if self.recents.is_empty() {
                ui.label("No recent ROMs");
                return;
              }

              let mut clicked = None;
              for (i, entry) in self.recents.iter().enumerate() {
                if ui.button(entry.to_str().unwrap_or_default()).clicked() {
                  clicked = Some(i);
                  break;
                }
              }

              if let Some(to_load_idx) = clicked {
                let to_load = self.recents.remove(to_load_idx).unwrap();
                self.load_rom(to_load);
              }
            })
          });

          ui.menu_button("🕹️ Emulation", |ui| {
            // TODO: ugly...
            if let Some((emu, state)) = &mut self.emu {
              match state {
                EmuState::Running => {
                  let pause =  ui.button("⏸ Pause");
                  ui.separator();
                  let reset = ui.button("🔄 Reset");
                  let stop = ui.button("⏹ Stop");
  
                  if pause.clicked() {
                    *state = EmuState::Paused;
                    self.sdl.audiodev.pause();
                  } else if reset.clicked() {
                    emu.emu_reset();
                    *state = EmuState::Running;
                    self.sdl.audiodev.clear();
                    self.sdl.audiodev.resume();
                  } else if stop.clicked() {
                    *state = EmuState::Stopped;
                    self.sdl.audiodev.clear();
                    self.sdl.audiodev.pause();
                  } else {
                    *state = EmuState::Running;
                  }
                }
                EmuState::Paused => {
                  let run =  ui.button("▶ Run");
                  ui.separator();
                  let reset = ui.button("🔄 Reset");
                  let stop = ui.button("⏹ Stop");
  
                  if run.clicked() {
                    *state = EmuState::Running;
                    self.sdl.audiodev.resume();
                  } else if reset.clicked() {
                    emu.emu_reset();
                    *state = EmuState::Running;
                    self.sdl.audiodev.clear();
                    self.sdl.audiodev.resume();
                  } else if stop.clicked() {
                    *state = EmuState::Stopped;
                    self.sdl.audiodev.clear();
                    self.sdl.audiodev.pause();
                  } else {
                    *state = EmuState::Paused;
                  }
                },
                EmuState::Stopped => {   
                  let run =  ui.button("▶ Run");
                  let reset = ui.button("🔄 Reset");
                  ui.separator();
                  ui.add_enabled(false, egui::Button::new("⏹ Stop"));
  
                  if run.clicked() {
                    emu.emu_reset();
                    *state = EmuState::Running;
                    self.sdl.audiodev.resume();
                  } else if reset.clicked() {
                    emu.emu_reset();
                    *state = EmuState::Running;
                    self.sdl.audiodev.clear();
                    self.sdl.audiodev.resume();
                  } else {
                    *state = EmuState::Stopped;
                  }
                }
                EmuState::Off => unreachable!()
              }
            } else {
              ui.add_enabled(false, egui::Button::new("▶ Run"));
              ui.separator();
              ui.add_enabled(false, egui::Button::new("🔄 Reset"));
              ui.add_enabled(false, egui::Button::new("⏹ Stop"));
            }
          });
          ui.menu_button("⚙️ Settings", |ui| {
            ui.checkbox(&mut self.keep_aspect_ratio, "📺 Keep Aspect Ratio");
            if ui.checkbox(&mut self.fullscreen, "🗔 Fullscreen").clicked() {
              ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }

            ui.menu_button("🖥️ Video Size", |ui| {
              for i in 1..6 {
                if ui.radio_value(&mut self.video_size, i, format!("{i}x")).clicked() {
                  let new_size = [256.0 * i as f32, 240.0 * i as f32];
                  // TODO: not quite right (not considering top panel row)
                  ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size.into()));
                }
              }
            });

            if ui.button("🎮 Keybinds").clicked() {
              self.windows.keybinds_open = true;
            }

            // TODO: palettes
            // TODO: bios path
            // TODO: all other settings
          });
          ui.menu_button("🐞 Debug", |ui| {
            let rom_info = egui::Button::new("💾 ROM information");

            if ui.add_enabled(running, rom_info).clicked() {
              self.windows.rom_info_open = true;
            }
          });
          ui.menu_button("❔ Help", |ui| {
            if ui.button("ℹ About").clicked() {
              self.windows.about_open = true;
            }
          });

          ui.separator();

          let volume_slider = egui::Slider::new(&mut self.settings.volume, 0.0..=100.0);
          ui.add(volume_slider);
        });
      });
    });

    egui::Window::new("🎮 Keybindings")
    .collapsible(true)
    .resizable([false, false])
    .open(&mut self.windows.keybinds_open)
    .show(ctx, |ui| {
      for key in &self.keymaps.keys {
        ui.columns_const::<2, _>(|ui| {
          // TODO: use const array of buttons string
          ui[0].label(format!("{:?}", key.1));

          if ui[1].button(format!("{:?}", key.0)).clicked() {
            self.keymaps.rebind_key = Some((key.0, key.1));
          }
        })
      }
    });

    egui::Window::new("💾 ROM information")
    .collapsible(true)
    .open(&mut self.windows.rom_info_open)
    .show(ctx, |ui| {
      let (emu, _) = self.emu.as_mut().unwrap();
      let header = emu.header();

      ui.columns_const::<2, _>(|ui| {
        ui[0].label("Header kind");
        ui[1].label(format!("{:?}", header.format));

        ui[0].label("Mapper ID");
        ui[1].label(header.mapper.to_string());
        ui[0].label("SubMapper ID");
        ui[1].label(header.submapper.to_string());

        ui[0].label("Region");
        ui[1].label(format!("{:?}", header.region));

        ui[0].label("Battery");
        ui[1].label(header.has_battery.to_string());
        ui[0].label("Trainer");
        ui[1].label(header.has_trainer.to_string());
        ui[0].label("CHR RAM");
        ui[1].label(header.has_chr_ram.to_string());

        ui[0].label("PRG size");
        ui[1].label(format!("{} KB", header.prg_size / 1024));

        ui[0].label("CHR size");
        ui[1].label(format!("{} KB", header.chr_size / 1024));

        ui[0].label("WRAM size");
        ui[1].label(format!("{} KB", header.wram_size / 1024));
      });
    });

    egui::Window::new("ℹ About")
    .collapsible(true)
    .open(&mut self.windows.about_open)
    .show(ctx, |ui| ui.vertical_centered(|ui| {
      ui.hyperlink_to("Nen Emulator", "https://github.com/Comba92/nen-emulator");
      ui.label("Developed by");
      ui.hyperlink_to("Comba92", "https://github.com/Comba92");
      ui.hyperlink_to("Report bugs or issues", "https://github.com/Comba92/nen-emulator/issues/new/choose")
    }));


    egui::CentralPanel::default()
    .show(ctx, |ui| {
      ui.vertical_centered(|ui| {
        let img = egui::Image::new(&self.tex)
        .maintain_aspect_ratio(self.keep_aspect_ratio)
        .fit_to_exact_size(ui.max_rect().size());
      
        ui.add(img);
      });
    });

    // input handling
    let has_run_one_frame = ctx.input(|i| {
      // check for dropped files
      let files = &i.raw.dropped_files;
      if let Some(Some(path)) = files.first().map(|f| &f.path) {
        self.load_rom(path);
      }

      let mut should_handle_input = true;

      // handle rebind if in rebind window
      if let Some(key_to_rebind) = &mut self.keymaps.rebind_key {
        for key_pressed in &i.keys_down {
          let entry_idx = self.keymaps.keys.iter()
            .position(|x| x.1 == key_to_rebind.1)
            .unwrap();
          self.keymaps.keys[entry_idx].0 = *key_pressed;
          self.keymaps.rebind_key = None;

          // we only set the first one
          should_handle_input = false;
          break;
        }
      }
      

      if let Some((emu, state)) = &mut self.emu {
        if *state != EmuState::Running { return false; }

        // run one emulation frame
        self.dt += i.stable_dt;
        if self.dt > self.fps {
          self.dt -= self.fps;

          if should_handle_input {
            emu.joypad.buttons = joypad::Button::empty();
            for key_pressed in &i.keys_down {
              if let Some((_, emu_key)) = self.keymaps.keys.iter().find(|x| x.0 == *key_pressed) {
                emu.set_button(*emu_key, true);
              }
            }
          }

          emu.step_until_vblank();
          let audiodev = &mut self.sdl.audiodev;
          audiodev.queue_audio(emu.get_audio()).unwrap();

          while audiodev.size()/2 < audiodev.spec().samples as u32 {
            // run for another frame
            emu.step_until_vblank();
            audiodev.queue_audio(emu.get_audio()).unwrap();
          }

          emu.get_video_rgba(self.framebuf.as_raw_mut());

          // sadly we have to clone the framebuf
          self.tex.set(self.framebuf.clone(), TEX_OPTS);
        }
        true
      } else {
        false
      }      
    });

    if has_run_one_frame { ctx.request_repaint(); }
  }
}