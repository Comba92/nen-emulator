use std::{
    fs,
    io::{Read, Write},
    path,
    sync::{self, Arc, Mutex},
    thread, time,
};

use nenemu_core::{emu::NesEmulator, joypad::JoypadBtn, utils::RingBuffer};
use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    controller::{Axis, Button},
    event::{Event, WindowEvent},
    keyboard::Keycode,
    pixels::Color,
    pixels::PixelFormatEnum,
    render::ScaleMode,
};
const AXIS_DEAD_ZONE: i16 = 10_000;

struct AudioHandler {
    emu: Arc<Mutex<NesEmulator>>,
}
impl AudioCallback for AudioHandler {
    type Channel = f32;

    fn callback(&mut self, audio_out: &mut [Self::Channel]) {
        let mut emu_lock = self.emu.lock().unwrap();
        emu_lock.put_audio_f32(audio_out);

        // let (right, left) = emu_lock.get_audio_f32(audio_out.len());
        // let right_amt = right.len();
        // audio_out[..right_amt].copy_from_slice(right);

        // if let Some(left) = left {
        //     audio_out[right_amt..].copy_from_slice(left);
        // }
    }
}

fn arc_mutex<T>(inner: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(inner))
}

fn save_battery(rom_path: &path::PathBuf, emu_lock: &sync::MutexGuard<NesEmulator>) {
    if let Some(sram) = emu_lock.save_battery() {
        let mut save_path = rom_path.clone();
        save_path.set_extension("sram");

        let mut file = fs::File::create(&save_path).unwrap();
        // let mut writer = BufWriter::new(file);
        // writer.write_all(sram).unwrap();
        file.write_all(sram).unwrap();
        println!("Battery saved to {save_path:?}");
    }
}

fn load_battery(rom_path: &path::PathBuf, emu_lock: &mut sync::MutexGuard<NesEmulator>) {
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
}

fn main() {
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let audio = sdl.audio().unwrap();
    let mut events = sdl.event_pump().unwrap();
    let controller = sdl.game_controller().unwrap();
    let mut controllers = Vec::new();
    // let timer = sdl.timer().unwrap();

    let window = video
        .window("NesEmu", 256 * 3, 240 * 3)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    canvas.set_logical_size(256, 240).unwrap();
    let texture_creator = canvas.texture_creator();
    let mut tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256, 240)
        .unwrap();
    tex.set_scale_mode(ScaleMode::Nearest);

    let debug_window = video
        .window("Debug", 256 * 2 * 2, 240 * 2 * 2)
        .resizable()
        .build()
        .unwrap();
    let mut debug_canvas = debug_window.into_canvas().build().unwrap();
    let debug_texture_creator = debug_canvas.texture_creator();
    let mut debug_tex = debug_texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256 * 2, 240 * 2)
        .unwrap();
    debug_tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

    let bios = include_bytes!("../../nenemu_core/utils/disksys.rom");
    let mut rom_path = path::PathBuf::from("roms/donkey kong.nes");

    let emu = NesEmulator::load_bios_only(Some(bios)).unwrap();
    // let emu = NesEmulator::load_rom_from_file(&rom_path, Some(bios)).unwrap();

    let emu = arc_mutex(emu);
    let emu_shared_clone = Arc::clone(&emu);

    let audiospec = AudioSpecDesired {
        channels: Some(1),
        freq: Some(48000),
        samples: Some(800),
    };

    let audiocb = audio
        .open_playback(None, &audiospec, move |_| AudioHandler {
            emu: emu_shared_clone,
        })
        .unwrap();
    audiocb.resume();

    println!("{:?}", audiocb.spec());

    let frame_rate = time::Duration::from_secs_f32(1.0 / 144.0);
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
                            save_battery(&rom_path, &emu_lock);

                            *emu_lock = res;
                            rom_path = path::PathBuf::from(filename);
                            println!("{:?}", emu_lock.header());

                            load_battery(&rom_path, &mut emu_lock);
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
                            #[cfg(feature = "savestates")]
                            Keycode::NUM_9 => emu_lock.savestate("./save.tmp").unwrap(),
                            #[cfg(feature = "savestates")]
                            Keycode::NUM_8 => {
                                emu_lock.loadstate("./save.tmp").unwrap();
                            }
                            Keycode::R => {
                                save_battery(&rom_path, &emu_lock);
                                emu_lock.emu_reset();
                                load_battery(&rom_path, &mut emu_lock);
                            }

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

                Event::ControllerButtonDown { button, .. } => {
                    let mut emu_lock = emu.lock().unwrap();
                    match button {
                        Button::DPadUp => emu_lock.set_button(JoypadBtn::Up, true),
                        Button::DPadLeft => emu_lock.set_button(JoypadBtn::Left, true),
                        Button::DPadDown => emu_lock.set_button(JoypadBtn::Down, true),
                        Button::DPadRight => emu_lock.set_button(JoypadBtn::Right, true),
                        Button::A => emu_lock.set_button(JoypadBtn::A, true),
                        Button::X => emu_lock.set_button(JoypadBtn::B, true),
                        Button::Start => emu_lock.set_button(JoypadBtn::Start, true),
                        Button::Back => emu_lock.set_button(JoypadBtn::Select, true),
                        _ => {}
                    }
                }

                Event::ControllerButtonUp { button, .. } => {
                    let mut emu_lock = emu.lock().unwrap();
                    match button {
                        Button::DPadUp => emu_lock.set_button(JoypadBtn::Up, false),
                        Button::DPadLeft => emu_lock.set_button(JoypadBtn::Left, false),
                        Button::DPadDown => emu_lock.set_button(JoypadBtn::Down, false),
                        Button::DPadRight => emu_lock.set_button(JoypadBtn::Right, false),
                        Button::A => emu_lock.set_button(JoypadBtn::A, false),
                        Button::X => emu_lock.set_button(JoypadBtn::B, false),
                        Button::Start => emu_lock.set_button(JoypadBtn::Start, false),
                        Button::Back => emu_lock.set_button(JoypadBtn::Select, false),
                        _ => {}
                    }
                }

                Event::ControllerAxisMotion {
                    axis: Axis::LeftX,
                    value,
                    ..
                } => {
                    let mut emu_lock = emu.lock().unwrap();

                    if value > AXIS_DEAD_ZONE {
                        emu_lock.set_button(JoypadBtn::Right, true);
                    } else if value < -AXIS_DEAD_ZONE {
                        emu_lock.set_button(JoypadBtn::Left, true);
                    } else {
                        emu_lock.set_button(JoypadBtn::Left, false);
                        emu_lock.set_button(JoypadBtn::Right, false);
                    }
                }
                Event::ControllerAxisMotion {
                    axis: Axis::LeftY,
                    value,
                    ..
                } => {
                    let mut emu_lock = emu.lock().unwrap();

                    if value > AXIS_DEAD_ZONE {
                        emu_lock.set_button(JoypadBtn::Down, true);
                    } else if value < -AXIS_DEAD_ZONE {
                        emu_lock.set_button(JoypadBtn::Up, true);
                    } else {
                        emu_lock.set_button(JoypadBtn::Up, false);
                        emu_lock.set_button(JoypadBtn::Down, false);
                    }
                }

                Event::ControllerDeviceAdded { which, .. } => match controller.open(which) {
                    Ok(controller) => {
                        println!("Found controller: {}\n", controller.name());
                        controllers.push(controller);
                    }
                    Err(e) => {
                        eprintln!("A controller was connected, but I couldn't initialize it: {e}\n")
                    }
                },
                _ => {}
            }
        }

        canvas.set_draw_color(Color::GREY);
        canvas.clear();

        {
            let mut emu_lock = emu.lock().unwrap();

            if emu_lock.audio_queued(48000) < 1024 {
                emu_lock.step_until_frame_ready().unwrap();

                tex.with_lock(None, |pixels, _| {
                    pixels.copy_from_slice(emu_lock.get_video_rgba());
                })
                .unwrap();

                // if emu_lock.is_frame_ready() {
                //     video_chain.push(emu_lock.get_video_rgba().clone());
                // }
            }

            debug_canvas.set_draw_color(Color::GREY);
            debug_canvas.clear();
            debug_tex
                .with_lock(None, |pixels, _| {
                    emu_lock.get_nametables_rgba(pixels);
                })
                .unwrap();
            debug_canvas.copy(&debug_tex, None, None).unwrap();
            debug_canvas.present();
        }

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        sleep_until_fps(frame_start, frame_rate);
    }
}

fn sleep_until_fps(frame_start: time::Instant, frame_rate: time::Duration) {
    let frame_duration = frame_start.elapsed();
    if frame_duration < frame_rate {
        thread::sleep(frame_rate - frame_duration);
    }
}
