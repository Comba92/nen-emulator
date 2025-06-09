use std::{io::Read, sync::Arc, time::{Duration, Instant}};

use eframe::egui;
use nen_emulator::{Emulator, JoypadButton};

const TEX_OPTS: egui::TextureOptions = egui::TextureOptions {
  magnification: egui::TextureFilter::Nearest,
  minification: egui::TextureFilter::Nearest,
  wrap_mode: egui::TextureWrapMode::ClampToEdge,
  mipmap_mode: None,
};

const FRAME_MS: f32 = 1.0 / 60.0;

fn main() {
  let opts = eframe::NativeOptions {
    centered: true,
    viewport: egui::ViewportBuilder::default()
      .with_drag_and_drop(true)
      .with_inner_size(egui::vec2(800.0, 600.0))
      .with_title("NenEmu"),
    vsync: true,

    ..Default::default()
  };

  // let (send, recv) = std::sync::mpsc::channel();
  // let emu_thread = std::thread::spawn(|| {

  // });

  eframe::run_native("NenEmu", opts, Box::new(
    |c| Ok(AppCtx::new(c))
  )).unwrap();
}

#[derive(Default, PartialEq)]
enum AppState {
  EmuRunning, EmuPaused, #[default] EmuStopped,
}

#[derive(Default)]
struct AppCtx {
  emu: Box<Emulator>,
  state: AppState,
  
  video_tex: Option<egui::TextureHandle>,
  
  current_rom: String,
  recent_roms: Vec<String>,
  should_close: bool,

  show_bugs_wnd: bool,
  show_about_wnd: bool,
  show_closing_wnd: bool,
  is_fullscreen: bool,

  frame_dt: f32,
  emu_time: Duration,
  render_time: Duration,
}

impl AppCtx {
  pub fn new(c: &eframe::CreationContext) -> Box<Self> {
    let mut emu = Box::new(Emulator::default());

    let frame = emu.get_frame_rgba();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &frame.buffer);
    let image_data = egui::ImageData::Color(Arc::new(color_image));
    let tex = c.egui_ctx.load_texture("tex", image_data, TEX_OPTS);


    let app = Box::new(Self {
      video_tex: Some(tex),
      emu,
      ..Default::default()
    });

    app
  }

  fn render_top_bar(&mut self, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top")
    .exact_height(20.0)
    .show(ctx, |ui| {
      egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
          if ui.button("Open...").clicked() {
            // TODO: open file dialog
          }
          ui.menu_button("Recents", |ui| {
            for rom in self.recent_roms.iter().rev() {
              if ui.button(rom).clicked() {
                // TODO: run rom with file dialog
              }
            }
            if ui.button("Clear").clicked() {
              self.recent_roms.clear();
            }
          });
          ui.menu_button("Savestates", |ui| {
            if ui.button("Quicksave").clicked() {
              // TODO: save game to current dir
            }
            if ui.button("Quickload").clicked() {
              // TODO: load game from current dir
            }
            if ui.button("Save...").clicked() {
              // TODO: open file dialog
            }
            if ui.button("Load...").clicked() {
              // TODO: open file dialog
            }
            ui.menu_button("Slot", |ui| {
              // TODO: radio
            });
          });
          if ui.button("Screenshot").clicked() {
            // TODO: take screenshot
            // egui has functionality for this
          }
          if ui.button("Quit").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
          }
        });
      

        ui.menu_button("Emulation", |ui| {
          match self.state {
            AppState::EmuRunning => {
              if ui.button("Pause").clicked() {
                self.state = AppState::EmuPaused;
                ui.close_menu();
              }
            }
            AppState::EmuPaused => {
              if ui.button("Resume").clicked() {
                self.state = AppState::EmuRunning;
                ui.close_menu();
              }
            }
            AppState::EmuStopped => ui.disable(),
          }

          if ui.button("Reset").clicked() {
            self.save_sram();
            self.emu.reset();
            ui.close_menu();
          }
          if ui.button("Force save SRAM").clicked() {
            self.save_sram();
            ui.close_menu();
          }
          if ui.button("Reload ROM").clicked() {
            self.save_sram();
            self.open_rom(&self.current_rom.clone());
            ui.close_menu();
          }
          if ui.button("Power OFF").clicked() {
            self.emu = Default::default();
            self.state = AppState::EmuStopped;
            self.save_sram();
            ui.close_menu();
          }
        });

        ui.menu_button("View", |ui| {
          ui.menu_button("Video size", |ui| {
            // TODO: radio
          });
          if ui.button("Fullscreen").clicked() {
            self.is_fullscreen = !self.is_fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
            ui.close_menu();
          }
        });
        ui.menu_button("Settings", |ui| {
          if ui.button("Keyboard binds").clicked() {

          }
          if ui.button("Controller binds").clicked() {

          }
          if ui.button("NES color palette").clicked() {

          }
          if ui.button("Folders").clicked() {
            
          }
        });
        ui.menu_button("Debug", |ui| {
          if ui.button("Registers viewer").clicked() {
            
          }
          if ui.button("Memory viewer").clicked() {
            
          }
          if ui.button("Tilemap viewer").clicked() {
            
          }
          if ui.button("Tileset viewer").clicked() {
            
          }
          if ui.button("Sprites viewer").clicked() {
            
          }
          if ui.button("Palette viewer").clicked() {
            
          }
          if ui.button("Rom header info").clicked() {
            
          }
        });
        ui.menu_button("Help", |ui| {
          if ui.button("About").clicked() {
            self.show_about_wnd = true;
            ui.close_menu();
          }
          if ui.button("Report bugs").clicked() {
            self.show_bugs_wnd = true;
            ui.close_menu();
          }
        }); 

        ui.label(format!("Emu time: {:?}, Render Time: {:?}", self.emu_time, self.render_time.saturating_sub(self.emu_time)))
      });
    });
  }

  fn render_windows(&mut self, ctx: &egui::Context) {
    egui::Window::new("About")
    .open(&mut self.show_about_wnd)
    .show(ctx, |ui| {
      ui.label("Made by Comba92");
      ui.hyperlink_to("NenEmulator Github", "https://github.com/Comba92/nen-emulator");
      ui.hyperlink_to("Comba92 Website", "https://comba92.github.io/");
    });

    egui::Window::new("Report bugs")
    .open(&mut self.show_bugs_wnd)
    .show(ctx, |ui| {
      ui.hyperlink_to("Report bugs on the github issues page: ", "https://github.com/Comba92/nen-emulator/issues")
    });

    let mut show_closing_wnd = self.show_closing_wnd;
    egui::Window::new("Confirm quitting?")
    .open(&mut show_closing_wnd)
    .show(ctx, |ui| {
      ui.horizontal(|ui| {
        if ui.button("Yes").clicked() {
          ctx.send_viewport_cmd(egui::ViewportCommand::Close);
          self.save_sram();
          self.should_close = true;
        }
        if ui.button("No").clicked() {
          self.show_closing_wnd = false;
        }
      });
    });
  }

  fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
    ctx.input(|i| {
      self.emu.clear_all_joypad_btns();
      for key in &i.keys_down {
        match key {
          egui::Key::A => self.emu.set_joypad_btn(JoypadButton::left),
          egui::Key::D => self.emu.set_joypad_btn(JoypadButton::right),
          egui::Key::W => self.emu.set_joypad_btn(JoypadButton::up),
          egui::Key::S => self.emu.set_joypad_btn(JoypadButton::down),
          egui::Key::J => self.emu.set_joypad_btn(JoypadButton::a),
          egui::Key::K => self.emu.set_joypad_btn(JoypadButton::b),
          egui::Key::M => self.emu.set_joypad_btn(JoypadButton::start),
          egui::Key::N => self.emu.set_joypad_btn(JoypadButton::select),
          _ => {}
        }
      }
    });
  }

  fn handle_dropped_file(&mut self, ctx: &egui::Context) {
    ctx.input(|i| {
      let files = &i.raw.dropped_files;
      if files.len() == 1 {
        // TODO: handle errors correctly
        // this only works on native

        let rom_path = files[0].path.as_ref().unwrap()
          .clone()
          .into_os_string()
          .into_string()
          .unwrap();
        self.open_rom(&rom_path);
      }
    });
  }
  
  fn open_rom(&mut self, rom_path: &str) {
    let rom_file: std::fs::File = std::fs::File::open(&rom_path).unwrap();
    let mut reader = std::io::BufReader::new(rom_file);
    let mut rom_bytes = Vec::new();
    reader.read_to_end(&mut rom_bytes).unwrap();

    // TODO: ask user if should close/save current game?
    if let Ok(new_emu) = Emulator::new(&rom_bytes) {
      self.save_sram();
      self.emu = new_emu;
      self.state = AppState::EmuRunning;
      self.current_rom = rom_path.to_string();
      self.recent_roms.push(rom_path.to_string());
    }
  }

  fn save_sram(&mut self) {
    // TODO
  }
}

impl eframe::App for AppCtx {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    if ctx.input(|i| i.viewport().close_requested()) {
      if !self.should_close {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.show_closing_wnd = true;
      }
    }
    
    let render_start = Instant::now();

    match self.state {
      AppState::EmuRunning => {
        self.frame_dt += ctx.input(|i| i.stable_dt);
        ctx.request_repaint_after_secs(FRAME_MS.min(0.1));
        if self.frame_dt >= FRAME_MS {
          let emu_start = Instant::now();
          self.emu.step_until_vblank();
          let _ = self.emu.get_samples();
          self.frame_dt -= FRAME_MS;  

          let frame = self.emu.get_frame_rgba();
          let color_image = egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &frame.buffer);
          let image_data = egui::ImageData::Color(Arc::new(color_image));
          self.video_tex.as_mut().unwrap().set(image_data, TEX_OPTS);
  
          self.emu_time = emu_start.elapsed();
        }
      }
      AppState::EmuPaused  => {}
      AppState::EmuStopped => {}
    }

    self.handle_dropped_file(ctx);
    self.handle_keyboard_input(ctx);
    
    self.render_top_bar(ctx);
    self.render_windows(ctx);

    egui::CentralPanel::default().show(ctx, |ui| {
      ui.centered_and_justified(|ui| {
        let img = egui::Image::new(self.video_tex.as_ref().unwrap())
          .maintain_aspect_ratio(true)
          .fit_to_exact_size(ui.max_rect().size());
        ui.add(img);
      })
    });

    self.render_time = render_start.elapsed();
  }
}