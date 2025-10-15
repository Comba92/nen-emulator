use std::{collections::VecDeque, fs, path::{Path, PathBuf}, time::{Duration, Instant}};

use eframe::egui;
use nes_emulator::{emu::{self, Emu}, joypad, NesPalette};

const TEX_OPTS: egui::TextureOptions = egui::TextureOptions {
  magnification: egui::TextureFilter::Nearest,
  minification: egui::TextureFilter::Nearest,
  wrap_mode: egui::TextureWrapMode::ClampToEdge,
  mipmap_mode: None,
};

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

  eframe::run_native(APP_NAME,
  opts,
  Box::new(
    |c| Ok(AppCtx::new(c))
  )).unwrap();
}

fn file_dialog(prompt: &str, requires: &str, extensions: &[&str]) -> Option<PathBuf> {
  rfd::FileDialog::new()
    .set_can_create_directories(true)
    .set_title(prompt)
    .add_filter(requires, extensions)
    .pick_file()
}

fn buffered_read<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, GenericError> {
  use std::io::Read;

  let mut bytes = Vec::new();
  let file = std::fs::File::open(path)?;
  let mut reader = std::io::BufReader::new(file);
  reader.read_to_end(&mut bytes)?;
  Ok(bytes)
}

fn ring_push_front<T>(queue: &mut VecDeque<T>, val: T, limit: usize) {
  queue.push_front(val);
  queue.truncate(limit);
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Default)]
struct AppWndCtx {
  keybinds_open: bool,
  rom_info_open: bool,
  about_open: bool,
  settings_open: bool,
  message_open: Option<(bool, std::time::Instant, GenericError)>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Default)]
struct AppCfg {
  keep_aspect_ratio: bool,
  fullscreen: bool,
  hide_cursor: bool,

  settings: emu::Settings,
  battery_saving: bool,
  bios_path: Option<PathBuf>,

  windows: AppWndCtx,
  recent_roms: VecDeque<PathBuf>,
  palettes: VecDeque<NesPalette>,
  keymaps: KeyMap,
}
impl AppCfg {
  pub fn new() -> Self {
    Self {
      keep_aspect_ratio: true,
      settings: emu::Settings::new(),
      ..Default::default()
    }
  }
}

struct AppCtx {
  emu: Option<(emu::Emu, EmuState)>,
  dt: f32,
  fps: f32,
  framebuf: egui::ColorImage,
  tex: egui::TextureHandle,

  bios: Option<Vec<u8>>,
  cfg: AppCfg,
  sdl: SdlCtx,
}
impl AppCtx {
  pub fn new(c: &eframe::CreationContext) -> Box<Self> {
    let img = egui::ColorImage::filled([256, 240], egui::Color32::TRANSPARENT);
    let tex = c.egui_ctx.load_texture("tex", img.clone(), TEX_OPTS);
    let sdl = SdlCtx::default();

    #[cfg(feature = "serde")]
    let cfg = if let Some(storage) = c.storage {
      eframe::get_value(storage, eframe::APP_KEY).unwrap_or_else(|| AppCfg::new())
    } else {
      AppCfg::new()
    };

    #[cfg(not(feature = "serde"))]
    let cfg = AppCfg::new();

    let bios = if let Some(path) = &cfg.bios_path  {
      buffered_read(path).ok()
    } else { None };

    Box::new(Self {
      emu: None,
      dt: 0.0,
      fps: 0.0,
      framebuf: img,
      tex,

      bios,
      cfg,
      sdl,
    })
  }

  fn show_message(&mut self, e: GenericError) {
    self.cfg.windows.message_open = Some((true, Instant::now(), e));
  }

  fn load_rom<P: AsRef<Path>>(&mut self, path: P) {
    let bios = if let Some(bytes) = &self.bios {
      Some(bytes.as_slice())
    } else {
      None
    };

    let res = emu::Emu::load_rom_from_file(&path, bios);
    match res {
      Ok(mut new_emu) => {
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

        new_emu.settings = self.cfg.settings.clone();
        if let Some(pal) = self.cfg.palettes.front() {
          new_emu.palette = pal.clone();
        }
        self.emu = Some((new_emu, new_state));
        ring_push_front(&mut self.cfg.recent_roms, path.as_ref().to_path_buf(), 12);
      }
      Err(e) => {
        self.show_message(e);
      }
    }
  }

  fn load_palette<P: AsRef<Path>>(&mut self, path: P) {
    _ = buffered_read(path)
    .map(|bytes| {
      NesPalette::from_pal_file(&bytes)
      .ok_or("not a valid NES palette file")
      .map(|pal| ring_push_front(&mut self.cfg.palettes, pal, 20))
      .map(|_| self.show_message("palette loaded".into()))
    })
    .map_err(|e| self.show_message(e));

    if let Some((emu, _)) = &mut self.emu {
      if let Some(pal) = self.cfg.palettes.front() {
        emu.palette = pal.clone();
      }
    }
  }

  #[cfg(feature = "serde")]
  fn get_states_dir(&self) -> PathBuf {
    let mut dir = eframe::storage_dir(APP_NAME).unwrap();
    dir.push("states");
    dir
  }

  #[cfg(feature = "serde")]
  fn get_rom_states_dir(&self) -> PathBuf {
    let mut dir = self.get_states_dir();
    let current_rom = self.cfg.recents.front().unwrap();
    dir.push(current_rom.file_stem().unwrap());
    dir
  }

  #[cfg(feature = "serde")]
  fn save_state(&self, name: &str) {
    let mut dir = self.get_states_dir();
    
    let current_rom = self.cfg.recents.front().unwrap();
    dir.push(current_rom.file_stem().unwrap());

    fs::create_dir_all(&dir);

    dir.push(name);
    dir.set_extension("state");
    match self.get_emu().savestate(dir) {
      Err(e) => self.show_error(e),
      _ => {}
    }
  }

  #[cfg(feature = "serde")]
  fn load_state(&mut self, name: &str) {
    let mut dir = self.get_rom_states_dir();

    dir.push(name);
    dir.set_extension("state");
    match self.get_emu_mut().loadstate(dir) {
      Err(e) => self.show_error(e),
      _ => {}
    }
  }

  fn get_emu(&self) -> &emu::Emu {
    &self.emu.as_ref().unwrap().0
  }

  fn get_emu_mut(&mut self) -> &mut emu::Emu {
    &mut self.emu.as_mut().unwrap().0
  }
}

impl eframe::App for AppCtx {
  #[cfg(feature = "serde")]
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
    eframe::set_value(storage, eframe::APP_KEY, &self.cfg);
  }

  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let running = self.emu.is_some();

    egui::TopBottomPanel::top("top").show_separator_line(true)
    .show(ctx, |ui| {
      egui::MenuBar::new()
      .ui(ui, |content| {
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
            });

            #[cfg(feature = "serde")]
            ui.add_enabled_ui(running, |ui| ui.menu_button("Savestates", |ui| {
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
                ctx.copy_text(self.get_states_dir().to_string_lossy().into_owned());
              }

              ui.separator();

              if ui.button("🗑 Clear game states").clicked() {
                // TODO: show modal
                let res = fs::remove_dir_all(self.get_rom_states_dir());
              }

              if ui.button("☠ Clear all states").clicked() {
                // TODO: show modal
                let dir = self.get_states_dir();
                let res = fs::remove_dir_all(dir);
              }
            }));

            if ui.button("❌ Quit").clicked() {
              // TODO: quit
            }
          });
          
          ui.menu_button("🕹 Emulation", |ui| {
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
          ui.menu_button("⚙ Settings", |ui| {
            ui.checkbox(&mut self.cfg.keep_aspect_ratio, "📺 Keep Aspect Ratio");
            if ui.checkbox(&mut self.cfg.fullscreen, "🖥 Fullscreen").clicked() {
              ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.cfg.fullscreen));
            }
            ui.checkbox(&mut self.cfg.hide_cursor, "🖱 Hide Cursor");

            // ui.menu_button("🖥 Video Size", |ui| {
            //   for i in 1..6 {
            //     if ui.radio_value(&mut self.cfg.video_size, i, format!("{i}x")).clicked() {
            //       should_resize = Some(i);
            //     }
            //   }
            // });

            if ui.button("🎮 Keybinds").clicked() {
              self.cfg.windows.keybinds_open = true;
            }

            if ui.button("🔧 Emulation").clicked() {
              self.cfg.windows.settings_open = true;
            }

            if ui.button("🎨 Theme").clicked() {
              // TODO: theming
            }
          });
          ui.menu_button("🐞 Debug", |ui| {
            let rom_info = egui::Button::new("💾 ROM information");

            if ui.add_enabled(running, rom_info).clicked() {
              self.cfg.windows.rom_info_open = true;
            }
            if ui.button("Run FDS BIOS").clicked() {
              if let Some(bios) = &self.bios {
                let emu = emu::Emu::load_bios_only(Some(bios.as_slice()));
                match emu {
                  Ok(emu) => self.emu = Some((emu, EmuState::Running)),
                  Err(e) => self.show_message(e),
                }
              } else {
                self.show_message("no BIOS ROM provided".into());
              }
            }
          });
          ui.menu_button("❔ Help", |ui| {
            if ui.button("ℹ About").clicked() {
              self.cfg.windows.about_open = true;
            }
            ui.hyperlink("🛠 Report bugs or issues");
          });

          if let Some((emu, _)) = &mut self.emu {
            let style = ui.style_mut();
            style.spacing.slider_width  *= 0.7;
            
            ui.separator();
            let volume_slider = egui::Slider::new(&mut emu.settings.volume, 0.0..=100.0);
            ui.label("🔊 Vol");
            ui.add(volume_slider);
          }
        });
      });
    });

    // if let Some(factor) = should_resize {
    //   let factor = factor as f32;
    //   let top_height = top_panel.response.rect.height();
    //   let new_size = [256.0 * factor, top_height + 240.0 * factor];
    //   ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size.into()));
    // }
    
    // TODO: palette viewer
    egui::Window::new("🔧 Settings")
    .collapsible(true)
    .resizable([true, true])
    .open(&mut self.cfg.windows.settings_open)
    .show(ctx, |ui| {
      ui.label("NOTE: all settings will be applied to the running game only when this window is closed.").highlight();
      ui.label("");

      if ui.button("Load FDS BIOS file...").clicked() {
        file_dialog("Select FDS BIOS file", "FDS BIOS", &["rom"])
        .map(|path| self.cfg.bios_path = Some(path));

        self.bios = if let Some(path) = &self.cfg.bios_path  {
          buffered_read(path).ok()
        } else { None };
      }
      if let Some(path) = &self.cfg.bios_path {
        ui.label(format!("BIOS selected at: {:?}", path));
      }

      if ui.button("🎨 Load palette file...").clicked() {
        let path = file_dialog("Select a NES palette file", "NES PAL file", &["pal"]);
        // TODO
      }

      ui.collapsing(" Misc", |ui| {
        ui.checkbox(&mut self.cfg.battery_saving, "Enable battery saving")
        .on_hover_text("This will dump work RAM in the same directory as the ROM's.");
        // TODO
      
        ui.checkbox(&mut self.cfg.settings.random_ram, "Randomize RAM at startup")
        .on_hover_text("Some games (such as Final Fantasy) use the random state of RAM at boot to seed their rngs");
        // TODO
      });

      ui.collapsing("📺 Video", |ui| {
        ui.checkbox(&mut self.cfg.settings.no_sprite_limit, "Show more than 8 sprites per scaline");
        ui.checkbox(&mut self.cfg.settings.disable_background, "Disable background tiles");
        ui.checkbox(&mut self.cfg.settings.disable_sprites, "Disable sprite tiles");
        ui.checkbox(&mut self.cfg.settings.pal_borders, "Show side PAL black borders");
      });
      ui.collapsing("🔊 Audio", |ui| {
        // TODO
      });
    })
    .or_else(|| {
      if let Some((emu, _)) = &mut self.emu {
        self.cfg.settings.volume = emu.settings.volume;
        emu.settings = self.cfg.settings.clone();
      }
      None
    });

    egui::Window::new("🎮 Keybindings")
    .collapsible(true)
    .resizable([true, true])
    .open(&mut self.cfg.windows.keybinds_open)
    .show(ctx, |ui| {
      const BTN_NAMES: &[&str] = &["Up", "Down", "Left", "Right", "A", "B", "Start", "Select"];

      for (key, btn_name) in self.cfg.keymaps.keys.iter().zip(BTN_NAMES.iter()) {
        ui.columns_const::<2, _>(|ui| {
          let col1 = ui[0].label(*btn_name);
          let col2 = ui[1].button(format!("{:?}", key.0));

          if let Some(rebind_key) = &self.cfg.keymaps.rebind_key {
            if rebind_key.1 == key.1 {
              col1.highlight();
              col2.highlight();
            } else if col2.clicked() {
              self.cfg.keymaps.rebind_key = Some((key.0, key.1));
            }
          } else if col2.clicked() {
            self.cfg.keymaps.rebind_key = Some((key.0, key.1));
          }
        });
      }

      ui.vertical_centered(|ui| if let Some(rebind_key) = &self.cfg.keymaps.rebind_key {
        ui.label(format!("Rebinding {:?}... Press any button, close window to cancel", rebind_key.1));
      });

      ui.set_clip_rect(ui.min_rect());
    }).or_else(|| {
      self.cfg.keymaps.rebind_key = None;
      None
    });

    egui::Window::new("💾 ROM information")
    .collapsible(true)
    .open(&mut self.cfg.windows.rom_info_open)
    .show(ctx, |ui| {
      let emu = &self.emu.as_ref().unwrap().0;
      let header = emu.header();

      ui.columns_const::<2, _>(|ui| {
        ui[0].label("Header kind");
        ui[1].label(format!("{:?}", header.format));

        // TODO: more mapper information
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

      ui.set_clip_rect(ui.min_rect());
    });

    egui::Window::new("ℹ About")
    .collapsible(true)
    .open(&mut self.cfg.windows.about_open)
    .show(ctx, |ui| ui.vertical_centered(|ui| {
      // TODO: do richtext shit
      ui.hyperlink_to("Nen Emulator", "https://github.com/Comba92/nen-emulator");
      ui.label("Developed by");
      ui.hyperlink_to("Comba92", "https://github.com/Comba92");
      ui.hyperlink_to("Report bugs or issues", "https://github.com/Comba92/nen-emulator/issues/new/choose");

      ui.set_clip_rect(ui.min_rect());
    }));

    if let Some((open, appeared, msg)) = &mut self.cfg.windows.message_open {
      let res = egui::Window::new("Message")
      .anchor(egui::Align2::RIGHT_BOTTOM, [-10.0, -10.0])
      .title_bar(false)
      .collapsible(false)
      .auto_sized()
      .fade_in(true)
      .fade_out(true)
      .open(open)
      .show(ctx, |ui| {
        ui.heading(msg.to_string());
      });

      const MSG_DELAY: Duration = Duration::from_secs(4);
      if appeared.elapsed() > MSG_DELAY  {
        *open = false;
      }

      if let None = res {
        self.cfg.windows.message_open = None;
      }
    }

    egui::CentralPanel::default()
    // .frame(egui::Frame::default().outer_margin(0).fill(egui::Color32::WHITE))
    .show(ctx, |ui| {
      ui.vertical_centered(|ui| {
        let img = egui::Image::new(&self.tex)
        .maintain_aspect_ratio(self.cfg.keep_aspect_ratio)
        .fit_to_exact_size(ui.max_rect().size());
      
        let screen = ui.add(img); 
        if running && self.cfg.hide_cursor {
          screen.on_hover_cursor(egui::CursorIcon::None);
        }
      });
    });

    // input handling
    let has_run_one_frame = ctx.input(|i| {
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

      let mut should_handle_input = true;

      // handle rebind if in rebind window
      if let Some(key_to_rebind) = &mut self.cfg.keymaps.rebind_key {
        for key_pressed in &i.keys_down {
          let entry_idx = self.cfg.keymaps.keys.iter()
            .position(|x| x.1 == key_to_rebind.1)
            .unwrap();
          self.cfg.keymaps.keys[entry_idx].0 = *key_pressed;
          self.cfg.keymaps.rebind_key = None;

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
              if let Some((_, emu_key)) = self.cfg.keymaps.keys.iter().find(|x| x.0 == *key_pressed) {
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