use std::sync::Arc;

use eframe::egui;
use nes_emulator::emu::Emu;

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

struct AppCtx {
  emu: Option<Emu>,
  dt: f32,
  framebuf: egui::ColorImage,
  tex: egui::TextureHandle,

  sdl: sdl2::Sdl,
  sdl_audio: sdl2::AudioSubsystem,
  sdl_audiodev: sdl2::audio::AudioQueue<i16>,
}
impl AppCtx {
  pub fn new(c: &eframe::CreationContext) -> Box<Self> {
    let img = egui::ColorImage::filled([256, 240], egui::Color32::TRANSPARENT);
    let tex = c.egui_ctx.load_texture("tex", img.clone(), TEX_OPTS);
    
    let sdl = sdl2::init().unwrap();
    let sdl_audio = sdl.audio().unwrap();
    let audiospec = sdl2::audio::AudioSpecDesired {
      channels: Some(1),
      freq: Some(48000),
      samples: None,
    };
    let sdl_audiodev = sdl_audio.open_queue::<i16, _>(None, &audiospec).unwrap();
    sdl_audiodev.resume();

    Box::new(Self {
      emu: None,
      dt: 0.0,
      framebuf: img,
      tex,
      sdl,
      sdl_audio,
      sdl_audiodev,
    })
  }
}

impl eframe::App for AppCtx {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    egui::TopBottomPanel::top("top")
    .show_separator_line(true)
    .show(ctx, |ui| {
      egui::MenuBar::new()
      .ui(ui, |content| {
        content.horizontal_wrapped(|ui| {
          ui.menu_button("File", |ui| {
            ui.button("Open...");
          });

          ui.menu_button("Emulation", |ui| {});
          ui.menu_button("Settings", |ui| {});
          ui.menu_button("About", |ui| {});          
        });
      });
    });

    egui::CentralPanel::default()
    .show(ctx, |ui| {
      ui.vertical_centered(|ui| {
        let img = egui::Image::new(&self.tex)
        .maintain_aspect_ratio(true)
        .fit_to_exact_size(ui.max_rect().size());
  
        ui.add(img);
      });
    });

    let has_run_one_frame = ctx.input(|i| {
      // check for dropped files
      let files = &i.raw.dropped_files;
      if let Some(Some(path)) = files.first().map(|f| &f.path) {
        let res = Emu::load_rom_from_file(path);
        match res {
          Ok(emu) => {
            // todo:
            self.emu = Some(emu);
          }
          Err(e) => {
            // todo: show error
          }
        }
      }

      // run one emulation frame
      self.dt += i.stable_dt;
      if self.dt > (1.0 / 60.0) {
        self.dt -= 1.0 / 60.0;
        if let Some(emu) = &mut self.emu {
          emu.emu_step_until_vblank();
          let audiodev = &mut self.sdl_audiodev;
          audiodev.queue_audio(emu.get_audio()).unwrap();

          while audiodev.size()/2 < audiodev.spec().samples as u32 {
              // run for another frame
  
              emu.emu_step_until_vblank();
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