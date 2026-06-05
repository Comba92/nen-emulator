use eframe::egui;

const TEX_OPTS: egui::TextureOptions = egui::TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Nearest,
    wrap_mode: egui::TextureWrapMode::ClampToEdge,
    mipmap_mode: None,
};

const APP_NAME: &'static str = "NenEmuTest";

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

struct AppCtx {}

impl eframe::App for AppCtx {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {}
}
