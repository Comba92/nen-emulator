use std::{
    fs,
    io::{Read, Write},
    path,
    sync::{Arc, Mutex},
    thread, time,
};

use nes_emulator::{emu::NesEmulator, joypad::JoypadBtn, utils::RingBuffer};
use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::{Event, WindowEvent},
    keyboard::Keycode,
    pixels::Color,
    pixels::PixelFormatEnum,
    render::ScaleMode,
};

struct AudioHandler {
    emu: Arc<Mutex<NesEmulator>>,
}
impl AudioCallback for AudioHandler {
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

fn emulation_thread_proc(
    emu: Arc<Mutex<NesEmulator>>,
    video_chain: Arc<Mutex<RingBuffer<Framebuf>>>,
    samples_needed: usize,
) {
    let frame_rate = time::Duration::from_secs_f32(1.0 / 288.0);
    loop {
        let frame_start = time::Instant::now();

        {
            let mut emu_lock = emu.lock().unwrap();
            while emu_lock.audio_queued() < samples_needed * 2 {
                while emu_lock.audio_queued() < samples_needed * 2 && !emu_lock.is_frame_ready() {
                    emu_lock.cpu_step();
                }
                // println!("[EMU THREAD]: enough audio ready");

                if emu_lock.is_frame_ready() {
                    // println!(
                    //     "[EMU THREAD]: push video to presentation thread: {}",
                    //     video_chain.lock().unwrap().queued()
                    // );

                    emu_lock.frame_ready = false;
                    let buf = emu_lock.get_video_rgba();
                    video_chain.lock().unwrap().push(Framebuf(buf.clone()));
                }
            }
        }

        let frame_duration = time::Instant::now() - frame_start;
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

fn arc_mutex<T>(inner: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(inner))
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
    tex.set_scale_mode(ScaleMode::Nearest);

    // let debug_window = video
    //     .window("Debug", 256 * 2 * 2, 240 * 2 * 2)
    //     .resizable()
    //     .build()
    //     .unwrap();
    // let mut debug_canvas = debug_window.into_canvas().build().unwrap();
    // let debug_texture_creator = debug_canvas.texture_creator();
    // let mut debug_tex = debug_texture_creator
    //     .create_texture_streaming(PixelFormatEnum::RGBA32, 256 * 2, 240 * 2)
    //     .unwrap();
    // debug_tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

    let bios = include_bytes!("../../utils/disksys.rom");
    let rom = include_bytes!("../../roms/donkey kong.nes");
    let mut rom_path = path::PathBuf::from("../../roms/donkey kong.nes");

    let emu = NesEmulator::load_rom_from_bytes(rom, Some(bios)).unwrap();
    // let mut frame_rate = (1.0 / emu.region().frame_rate() * 1000.0).round() as u64;
    let mut frame_rate = time::Duration::from_secs_f32(1.0 / 144.0);

    let video_chain = arc_mutex(nes_emulator::utils::RingBuffer::new(8));

    let emu = arc_mutex(emu);
    let emu_shared_clone1 = Arc::clone(&emu);
    let emu_shared_clone2 = Arc::clone(&emu);
    let video_chain_shared_clone = Arc::clone(&video_chain);

    let audiospec = AudioSpecDesired {
        channels: Some(1),
        freq: Some(44100),
        samples: Some(1024),
    };

    let audiocb = audio
        .open_playback(None, &audiospec, move |_| AudioHandler {
            emu: emu_shared_clone2,
        })
        .unwrap();

    let samples_needed = audiocb.spec().samples;
    let _ = thread::spawn(move || {
        emulation_thread_proc(
            emu_shared_clone1,
            video_chain_shared_clone,
            samples_needed as usize,
        )
    });

    audiocb.resume();
    println!("{:?}", audiocb.spec());

    'running: loop {
        // let frame_start = timer.ticks64();
        let frame_start = time::Instant::now();

        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::Window { win_event, .. } => match win_event {
                    WindowEvent::Close => break 'running,
                    _ => {}
                },
                Event::DropFile { filename, .. } => {
                    if filename.ends_with(".pal") {
                        let buf = fs::read(filename).unwrap();
                        emu.lock().unwrap().load_palette(&buf);
                        continue;
                    }

                    let new_emu = NesEmulator::load_rom_from_file(&filename, Some(bios));
                    match new_emu {
                        Ok(res) => {
                            let mut emu_lock = emu.lock().unwrap();

                            // save current game battery
                            if let Some(sram) = emu_lock.save_battery() {
                                let mut save_path = rom_path.clone();
                                save_path.set_extension("sram");

                                let mut file = fs::File::create(&save_path).unwrap();
                                // let mut writer = BufWriter::new(file);
                                // writer.write_all(sram).unwrap();
                                file.write_all(sram).unwrap();
                                println!("Battery saved to {save_path:?}");
                            }

                            *emu_lock = res;
                            rom_path = path::PathBuf::from(filename);
                            println!("{:?}", emu_lock.header());

                            // load current game battery if any
                            let mut load_path = rom_path.clone();
                            load_path.set_extension("sram");

                            if let Ok(mut file) = fs::File::open(&load_path) {
                                let mut buf = Vec::new();
                                // let mut reader = BufReader::new(file);
                                // reader.read_to_end(&mut buf).unwrap();
                                file.read_to_end(&mut buf).unwrap();
                                let res = emu_lock.load_battery(&buf);
                                match res {
                                    Err(e) => eprintln!("{e}"),
                                    _ => println!("Battery loaded from {load_path:?}"),
                                }
                            }

                            // frame_rate =
                            //     (1.0 / emu_lock.region().frame_rate() * 1000.0).round() as u64;
                            // frame_rate = time::Duration::from_secs_f32(
                            //     1.0 / (emu_lock.region().frame_rate() + 1.0),
                            // );
                        }
                        Err(e) => eprintln!("{e}"),
                    }
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        let mut emu_lock = emu.lock().unwrap();
                        match keycode {
                            Keycode::Up => emu_lock.set_button(JoypadBtn::Up, true),
                            Keycode::Left => emu_lock.set_button(JoypadBtn::Left, true),
                            Keycode::Down => emu_lock.set_button(JoypadBtn::Down, true),
                            Keycode::Right => emu_lock.set_button(JoypadBtn::Right, true),
                            Keycode::S => emu_lock.set_button(JoypadBtn::A, true),
                            Keycode::A => emu_lock.set_button(JoypadBtn::B, true),
                            Keycode::W => emu_lock.set_button(JoypadBtn::Start, true),
                            Keycode::E => emu_lock.set_button(JoypadBtn::Select, true),
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
                        let mut emu_lock = emu.lock().unwrap();
                        match keycode {
                            Keycode::Up => emu_lock.set_button(JoypadBtn::Up, false),
                            Keycode::Left => emu_lock.set_button(JoypadBtn::Left, false),
                            Keycode::Down => emu_lock.set_button(JoypadBtn::Down, false),
                            Keycode::Right => emu_lock.set_button(JoypadBtn::Right, false),
                            Keycode::S => emu_lock.set_button(JoypadBtn::A, false),
                            Keycode::A => emu_lock.set_button(JoypadBtn::B, false),
                            Keycode::W => emu_lock.set_button(JoypadBtn::Start, false),
                            Keycode::E => emu_lock.set_button(JoypadBtn::Select, false),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        canvas.set_draw_color(Color::GREY);
        canvas.clear();

        {
            let mut video_lock = video_chain.lock().unwrap();
            if video_lock.queued() > 0 {
                // println!(
                //     "[PRESENT THREAD]: rendering video to texture: {}",
                //     video_lock.queued()
                // );

                let framebuf = video_lock.pop();
                tex.with_lock(None, |pixels, _| {
                    pixels.copy_from_slice(&framebuf.0);
                })
                .unwrap();
            }
        }

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        // debug_canvas.set_draw_color(Color::GREY);
        // debug_canvas.clear();
        // debug_tex
        //     .with_lock(None, |pixels, _| {
        //         emu_arc.lock().unwrap().get_nametables_rgba(pixels);
        //     })
        //     .unwrap();
        // debug_canvas.copy(&debug_tex, None, None).unwrap();
        // debug_canvas.present();

        // let frame_duration = timer.ticks64() - frame_start;
        // if frame_duration < frame_rate {
        //     timer.delay((frame_rate - frame_duration) as u32);
        // }
        let frame_duration = time::Instant::now() - frame_start;
        if frame_duration < frame_rate {
            thread::sleep(frame_rate - frame_duration);
        }
    }
}
