use std::{
    fs,
    io::{BufReader, BufWriter, Read, Write},
    path,
    sync::{Arc, Mutex, mpsc},
    thread, time,
};

use nes_emulator::{emu::Emu, joypad::JoypadBtn, utils::RingBuffer};
use sdl2::{
    audio::AudioCallback,
    event::{Event, WindowEvent},
    keyboard::Keycode,
    pixels::PixelFormatEnum,
};

struct AudioHandler {
    emu: Arc<Mutex<Emu>>,
}
impl AudioCallback for AudioHandler {
    type Channel = f32;

    fn callback(&mut self, audio_out: &mut [Self::Channel]) {
        let mut emu_lock = self.emu.lock().unwrap();
        let audio_needed = audio_out.len();

        let (right, left) = emu_lock.get_audio_amount_f32(audio_needed);
        let right_amt = right.len();
        audio_out[..right_amt].copy_from_slice(right);

        if let Some(left) = left {
            audio_out[right_amt..].copy_from_slice(left);
        }
    }
}

fn emu_thread_proc(emu: Arc<Mutex<Emu>>, video_chain: Arc<Mutex<RingBuffer<Framebuf>>>) {
    let mut buf = [0; 256 * 240 * 4];

    let frame_rate = time::Duration::from_secs_f32(1.0 / 576.0);
    loop {
        let start = time::Instant::now();

        {
            let mut emu_lock = emu.lock().unwrap();
            while emu_lock.audiobuf().queued() < 1024 * 8 {
                while emu_lock.audiobuf().queued() < 1024 * 8 && !emu_lock.frame_ready {
                    emu_lock.cpu_step();
                }
                // println!("[EMU THREAD]: enough audio ready");

                if emu_lock.frame_ready {
                    // println!("[EMU THREAD]: push video to presentation thread");

                    emu_lock.get_video_rgba(&mut buf);
                    video_chain.lock().unwrap().push(Framebuf(buf.clone()));
                    emu_lock.frame_ready = false;
                }
            }
        }

        let frame_duration = time::Instant::now() - start;
        if frame_duration < frame_rate {
            thread::sleep(frame_rate - frame_duration);
        }
    }
}

#[derive(Clone)]
struct Framebuf([u8; 256 * 240 * 4]);
impl Default for Framebuf {
    fn default() -> Self {
        Self([0; _])
    }
}

fn main() {
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let audio = sdl.audio().unwrap();
    let mut events = sdl.event_pump().unwrap();
    let timer = sdl.timer().unwrap();

    let window = video
        .window("NesEmu", 256 * 3, 240 * 3)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    canvas.set_logical_size(256, 240).unwrap();
    let texture_creator = canvas.texture_creator();
    let mut tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256, 240)
        .unwrap();
    tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

    // let debug_window = video.window("Debug", 256 * 2 * 2, 240 * 2 * 2)
    // .resizable()
    // .build().unwrap();
    // let mut debug_canvas = debug_window.into_canvas()
    //     .build().unwrap();
    // let debug_texture_creator  = debug_canvas.texture_creator();
    // let mut debug_tex = debug_texture_creator
    //     .create_texture_streaming(PixelFormatEnum::RGBA32, 256 * 2, 240 * 2)
    //     .unwrap();
    // debug_tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

    let bios = include_bytes!("../../utils/disksys.rom");
    let rom = include_bytes!("../../roms/donkey kong.nes");
    let emu = Emu::load_rom_from_bytes(rom, Some(bios)).unwrap();

    let mut rom_filename = "../roms/super mario.nes".to_string();

    let mut frame_rate = (1.0 / emu.region().frame_rate() * 1000.0).round() as u64;

    let video_chain = Arc::new(Mutex::new(nes_emulator::utils::RingBuffer::new(4)));

    let mut emu_arc = Arc::new(Mutex::new(emu));
    let emu_arc1 = emu_arc.clone();
    let emu_arc2 = emu_arc.clone();
    let video_arc = video_chain.clone();
    let emu_thread = thread::spawn(move || emu_thread_proc(emu_arc1, video_arc));

    let audiospec = sdl2::audio::AudioSpecDesired {
        channels: Some(1),
        freq: Some(44100),
        samples: Some(1024),
    };

    // let audiodev = audio.open_queue::<f32, _>(None, &audiospec).unwrap();
    let audiocb = audio
        .open_playback(None, &audiospec, move |_| AudioHandler { emu: emu_arc2 })
        .unwrap();

    audiocb.resume();

    // audiodev.resume();
    println!("{:?}", audiocb.spec());

    'running: loop {
        let frame_start = timer.ticks64();

        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::Window { win_event, .. } => match win_event {
                    WindowEvent::Close => break 'running,
                    _ => {}
                },
                Event::DropFile { filename, .. } => {
                    // if filename.ends_with(".pal") {
                    //     let buf = fs::read(filename).unwrap();
                    //     emu.load_palette(&buf);
                    //     continue;
                    // }

                    let new_emu = Emu::load_rom_from_file(&filename, Some(bios));
                    match new_emu {
                        Ok(res) => {
                            // // save current game battery
                            // if let Some(sram) = emu.save_battery() {
                            //     let mut save_path = path::PathBuf::from(&rom_filename);
                            //     save_path.set_extension("sram");

                            //     let file = fs::File::create(&save_path).unwrap();
                            //     let mut writer = BufWriter::new(file);
                            //     writer.write_all(sram).unwrap();
                            //     println!("Battery saved to {save_path:?}");
                            // }

                            rom_filename = filename;
                            let mut emu_lock = emu_arc.lock().unwrap();
                            *emu_lock = res;
                            println!("{:?}", emu_lock.header());

                            // load current game battery if any
                            // let mut load_path = path::PathBuf::from(&rom_filename);
                            // load_path.set_extension("sram");
                            // if let Ok(file) = fs::File::open(&load_path) {
                            //     let mut buf = Vec::new();
                            //     let mut reader = BufReader::new(file);
                            //     reader.read_to_end(&mut buf).unwrap();
                            //     let res = emu.load_battery(&buf);
                            //     match res {
                            //         Err(e) => eprintln!("{e}"),
                            //         _ => println!("Battery loaded from {load_path:?}"),
                            //     }
                            // }

                            frame_rate =
                                (1.0 / emu_lock.region().frame_rate() * 1000.0).round() as u64;
                        }
                        Err(e) => eprintln!("{e}"),
                    }
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        let mut emu_lock = emu_arc.lock().unwrap();
                        match keycode {
                            Keycode::W => emu_lock.set_button(JoypadBtn::Up, true),
                            Keycode::A => emu_lock.set_button(JoypadBtn::Left, true),
                            Keycode::S => emu_lock.set_button(JoypadBtn::Down, true),
                            Keycode::D => emu_lock.set_button(JoypadBtn::Right, true),
                            Keycode::K => emu_lock.set_button(JoypadBtn::A, true),
                            Keycode::J => emu_lock.set_button(JoypadBtn::B, true),
                            Keycode::M => emu_lock.set_button(JoypadBtn::Start, true),
                            Keycode::N => emu_lock.set_button(JoypadBtn::Select, true),
                            Keycode::NUM_0 => emu_lock.mapper.special_input(),
                            #[cfg(feature = "serde")]
                            Keycode::NUM_9 => emu_lock.savestate("./save.tmp").unwrap(),
                            #[cfg(feature = "serde")]
                            Keycode::NUM_8 => {
                                emu_lock.loadstate("./save.tmp").unwrap();
                                audiodev.clear();
                            }
                            Keycode::R => emu_lock.emu_reset(),
                            _ => {}
                        }
                    }
                }

                Event::KeyUp { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        let mut emu_lock = emu_arc.lock().unwrap();
                        match keycode {
                            Keycode::W => emu_lock.set_button(JoypadBtn::Up, false),
                            Keycode::A => emu_lock.set_button(JoypadBtn::Left, false),
                            Keycode::S => emu_lock.set_button(JoypadBtn::Down, false),
                            Keycode::D => emu_lock.set_button(JoypadBtn::Right, false),
                            Keycode::K => emu_lock.set_button(JoypadBtn::A, false),
                            Keycode::J => emu_lock.set_button(JoypadBtn::B, false),
                            Keycode::M => emu_lock.set_button(JoypadBtn::Start, false),
                            Keycode::N => emu_lock.set_button(JoypadBtn::Select, false),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        canvas.set_draw_color(sdl2::pixels::Color::GREY);
        canvas.clear();

        {
            let mut video_lock = video_chain.lock().unwrap();
            if video_lock.queued() > 0 {
                // println!("[PRESENT THREAD]: rendering video to texture");

                let (framebuf, _) = video_lock.take_available_contiguos(1);
                tex.with_lock(None, |pixels, _| {
                    pixels.copy_from_slice(&framebuf[0].0);
                })
                .unwrap();
            }
        }

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        // debug_canvas.set_draw_color(sdl2::pixels::Color::GREY);
        // debug_canvas.clear();

        // debug_tex.with_lock(None, |pixels, _| {
        //     emu.get_nametables_rgba(pixels);
        // }).unwrap();

        // debug_canvas.copy(&debug_tex, None, None).unwrap();
        // debug_canvas.present();

        let frame_duration = timer.ticks64() - frame_start;
        if frame_duration < frame_rate {
            timer.delay((frame_rate - frame_duration) as u32);
        }
    }
}
