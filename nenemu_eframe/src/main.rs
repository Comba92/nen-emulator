use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::{App, egui};
use nenemu_core::{emu::NesEmulator, utils::RingBuffer};
use std::{
    sync::{Arc, Mutex},
    thread, time,
};

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

    eframe::run_native(APP_NAME, opts, Box::new(|c| Ok(AppCtx::new(c)))).unwrap();
}

struct AudioHandler {
    emu: Arc<Mutex<NesEmulator>>,
}
impl sdl2::audio::AudioCallback for AudioHandler {
    type Channel = f32;

    fn callback(&mut self, audio_out: &mut [Self::Channel]) {
        let mut emu_lock = self.emu.lock().unwrap();

        let (right, left) = emu_lock.get_audio_f32(audio_out.len());
        let right_amt = right.len();
        audio_out[..right_amt].copy_from_slice(right);

        if let Some(left) = left {
            audio_out[right_amt..].copy_from_slice(left);
        }
    }
}

struct SdlCtx {
    sdl: sdl2::Sdl,
    audio: sdl2::AudioSubsystem,
    audiodev: sdl2::audio::AudioDevice<AudioHandler>,
    samplebuf_size: usize,
}

impl SdlCtx {
    pub fn new(sample_rate: usize, emu: Arc<Mutex<NesEmulator>>) -> Self {
        let sdl = sdl2::init().unwrap();
        let audio = sdl.audio().unwrap();
        let audiospec = sdl2::audio::AudioSpecDesired {
            channels: Some(1),
            freq: Some(sample_rate as i32),
            samples: Some(256),
        };

        let audiodev = audio
            .open_playback(None, &audiospec, |_| AudioHandler { emu })
            .unwrap();
        // audiodev.resume();

        let samplebuf_size = audiodev.spec().samples as usize;
        Self {
            sdl,
            audio,
            audiodev,
            samplebuf_size,
        }
    }
}

fn emulation_thread_proc(
    emu: Arc<Mutex<NesEmulator>>,
    video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
    samples_needed: usize,
) {
    let frame_rate = time::Duration::from_secs_f32(1.0 / 288.0);
    loop {
        let frame_start = time::Instant::now();

        {
            let mut emu_lock = emu.lock().unwrap();
            while emu_lock.audio_queued() < samples_needed * 2 {
                emu_lock
                    .step_until_samples_or_frame_ready(samples_needed * 2)
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

        let frame_duration = time::Instant::now() - frame_start;
        if frame_duration < frame_rate {
            thread::sleep(frame_rate - frame_duration);
        }
    }
}

struct AppCtx {
    // sdl: SdlCtx,
    emu: Arc<Mutex<NesEmulator>>,
    emu_thread: thread::JoinHandle<()>,
    video_chain: Arc<Mutex<RingBuffer<egui::ColorImage>>>,
    tex: Arc<Mutex<egui::TextureHandle>>,
    audio_stream: cpal::Stream,
    dt: f32,
}
impl AppCtx {
    pub fn new(c: &eframe::CreationContext) -> Box<Self> {
        let img = egui::ColorImage::filled([256, 240], egui::Color32::DARK_RED);
        let tex = c.egui_ctx.load_texture("emu_present", img, TEX_OPTS);
        let tex = Arc::new(Mutex::new(tex));

        let emu = NesEmulator::empty();
        let emu = Arc::new(Mutex::new(emu));
        // let sdl = SdlCtx::new(44100, Arc::clone(&emu));

        let video_chain = Arc::new(Mutex::new(RingBuffer::new(8)));

        println!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
        let available_hosts = cpal::available_hosts();
        println!("Available hosts:\n  {available_hosts:?}");

        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: 48000,
            buffer_size: cpal::BufferSize::Fixed(256),
        };

        let emu_arc = Arc::clone(&emu);
        let stream = device.build_output_stream(config, move |audio_out, _| {
            let mut emu_lock = emu_arc.lock().unwrap();

            let (right, left) = emu_lock.get_audio_f32(audio_out.len() / 2);
            for i in 0..right.len() {
                audio_out[2*i] = right[i];
                audio_out[2*i + 1] = right[i];
            }

            if let Some(left) = left {
                let audio_out = &mut audio_out[2*right.len()..];
                for i in 0..left.len() {
                    audio_out[2*i] = left[i];
                    audio_out[2*i + 1] = left[i];
                }
            }
        }, |err| eprintln!("{err}"), None).unwrap();

        stream.play().unwrap();

        let emu_arc = Arc::clone(&emu);
        let chain_arc = Arc::clone(&video_chain);
        let samples_needed = stream.buffer_size().unwrap() as usize;
        let emu_thread = thread::Builder::new()
            .name("emulation".into())
            .spawn(move || emulation_thread_proc(emu_arc, chain_arc, samples_needed))
            .unwrap();

        let res = Self {
            // sdl,
            emu,
            emu_thread,
            video_chain,
            audio_stream: stream,
            tex,
            dt: 0.0,
        };
        Box::new(res)
    }
}

impl eframe::App for AppCtx {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Hello!!!");

            let tex = self.tex.lock().unwrap();
            let img = egui::Image::new(&*tex)
                .maintain_aspect_ratio(true)
                .fit_to_exact_size(ui.max_rect().size());

            ui.add(img);
        });

        let stable_dt = ui.input(|i| {
            if !i.keys_down.is_empty() {
                println!("{:?}", i.keys_down);
            }

            i.stable_dt
        });

        let rom_path = ui.input(|i| {
            // check for dropped files
            let files = &i.raw.dropped_files;
            files.first().map(|f| &f.path).cloned()
        });

        if let Some(Some(rom_path)) = rom_path {
            let mut emu_lock = self.emu.lock().unwrap();
            *emu_lock = NesEmulator::load_rom_from_file(rom_path, None).unwrap();
        }

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

        {
            let mut video_lock = self.video_chain.lock().unwrap();
            if video_lock.queued() > 0 {
                let framebuf = std::mem::take(video_lock.pop());
                self.tex.lock().unwrap().set(framebuf, TEX_OPTS);
            }
        }

        const FPS: f32 = 1.0 / 144.0;
        ui.request_repaint_after_secs(FPS);
    }
}
