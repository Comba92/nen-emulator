use std::{
    fs,
    io::{Read, Write},
    path,
    sync::{self, Arc, Mutex},
    thread, time,
};

use nenemu_core::{
    emu::{self, NesEmulator},
    joypad::JoypadInput,
};
use sdl2::{
    audio::AudioCallback,
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

        // let AB = 1024.0;
        // let Ab = emu_lock.audio_queued() as f64;
        // let requested = AB - Ab;

        // const DELTA: f64 = 0.0005;
        // let dyn_rate = DELTA * (1.0 + (AB - 2.0 * Ab) / AB);
        // println!(
        //     "{} {} {requested} {dyn_rate} {}",
        //     AB,
        //     Ab,
        //     48000.0 * (1.0 + dyn_rate)
        // );

        // emu_lock.get_audio_f32_interp(audio_out.len(), (48000.0 + (1.0 * dyn_rate)) as usize);
        emu_lock.put_audio_f32(audio_out);
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

    let spec = sdl2::audio::AudioSpecDesired {
        freq: Some(48000),
        channels: Some(1),
        samples: Some(1024),
    };
    // let queue = audio.open_queue(None, &spec).unwrap();
    // queue.resume();

    let mut bios_path = None;
    let mut rom_path = path::PathBuf::new();

    // let emu = NesEmulator::load_bios_only(Some(bios)).unwrap();
    // let emu = NesEmulator::load_rom_from_file(&rom_path, Some(bios)).unwrap();
    let emu = NesEmulator::empty();

    let frame_rate = time::Duration::from_secs_f32(1.0 / emu.frame_rate());
    let emu = arc_mutex(emu);

    let emu_clone = emu.clone();
    let audiocb = audio
        .open_playback(None, &spec, |_| AudioHandler { emu: emu_clone })
        .unwrap();
    audiocb.resume();

    let mut adj_period = 3;
    let mut prev_y = 0.0;

    let mut w_values = [-1; 60];
    let mut w_index = 0;
    let mut out_freq = 48000.0;

    let mut rate_values = [0.0; 60];

    let mut frames = 0.0;

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
                        _ = emu.lock().unwrap().try_set_palette(&buf);
                        continue;
                    } else if filename.contains("disksys.rom") {
                        bios_path = Some(path::PathBuf::from(&filename));
                        continue;
                    }

                    let new_emu = NesEmulator::builder()
                        .with_rom_file(&filename)
                        .with_fds_bios_file(bios_path.as_ref())
                        .build();

                    match new_emu {
                        Ok(res) => {
                            let mut emu_lock = emu.lock().unwrap();

                            // save current game battery
                            save_battery(&rom_path, &emu_lock);

                            *emu_lock = res;
                            rom_path = path::PathBuf::from(filename);
                            println!("{:?}", emu_lock.rom_info());

                            load_battery(&rom_path, &mut emu_lock);
                        }
                        Err(e) => eprintln!("{e}"),
                    }
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        let mut emu_lock = emu.lock().unwrap();
                        match keycode {
                            Keycode::Up => emu_lock.set_button(JoypadInput::Up, true),
                            Keycode::Left => emu_lock.set_button(JoypadInput::Left, true),
                            Keycode::Down => emu_lock.set_button(JoypadInput::Down, true),
                            Keycode::Right => emu_lock.set_button(JoypadInput::Right, true),
                            Keycode::S => emu_lock.set_button(JoypadInput::A, true),
                            Keycode::A => emu_lock.set_button(JoypadInput::B, true),
                            Keycode::W => emu_lock.set_button(JoypadInput::Start, true),
                            Keycode::E => emu_lock.set_button(JoypadInput::Select, true),
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
                            Keycode::Up => emu_lock.set_button(JoypadInput::Up, false),
                            Keycode::Left => emu_lock.set_button(JoypadInput::Left, false),
                            Keycode::Down => emu_lock.set_button(JoypadInput::Down, false),
                            Keycode::Right => emu_lock.set_button(JoypadInput::Right, false),
                            Keycode::S => emu_lock.set_button(JoypadInput::A, false),
                            Keycode::A => emu_lock.set_button(JoypadInput::B, false),
                            Keycode::W => emu_lock.set_button(JoypadInput::Start, false),
                            Keycode::E => emu_lock.set_button(JoypadInput::Select, false),
                            _ => {}
                        }
                    }
                }

                Event::ControllerButtonDown { button, .. } => {
                    let mut emu_lock = emu.lock().unwrap();
                    match button {
                        Button::DPadUp => emu_lock.set_button(JoypadInput::Up, true),
                        Button::DPadLeft => emu_lock.set_button(JoypadInput::Left, true),
                        Button::DPadDown => emu_lock.set_button(JoypadInput::Down, true),
                        Button::DPadRight => emu_lock.set_button(JoypadInput::Right, true),
                        Button::A => emu_lock.set_button(JoypadInput::A, true),
                        Button::X => emu_lock.set_button(JoypadInput::B, true),
                        Button::Start => emu_lock.set_button(JoypadInput::Start, true),
                        Button::Back => emu_lock.set_button(JoypadInput::Select, true),
                        _ => {}
                    }
                }

                Event::ControllerButtonUp { button, .. } => {
                    let mut emu_lock = emu.lock().unwrap();
                    match button {
                        Button::DPadUp => emu_lock.set_button(JoypadInput::Up, false),
                        Button::DPadLeft => emu_lock.set_button(JoypadInput::Left, false),
                        Button::DPadDown => emu_lock.set_button(JoypadInput::Down, false),
                        Button::DPadRight => emu_lock.set_button(JoypadInput::Right, false),
                        Button::A => emu_lock.set_button(JoypadInput::A, false),
                        Button::X => emu_lock.set_button(JoypadInput::B, false),
                        Button::Start => emu_lock.set_button(JoypadInput::Start, false),
                        Button::Back => emu_lock.set_button(JoypadInput::Select, false),
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
                        emu_lock.set_button(JoypadInput::Right, true);
                    } else if value < -AXIS_DEAD_ZONE {
                        emu_lock.set_button(JoypadInput::Left, true);
                    } else {
                        emu_lock.set_button(JoypadInput::Left, false);
                        emu_lock.set_button(JoypadInput::Right, false);
                    }
                }
                Event::ControllerAxisMotion {
                    axis: Axis::LeftY,
                    value,
                    ..
                } => {
                    let mut emu_lock = emu.lock().unwrap();

                    if value > AXIS_DEAD_ZONE {
                        emu_lock.set_button(JoypadInput::Down, true);
                    } else if value < -AXIS_DEAD_ZONE {
                        emu_lock.set_button(JoypadInput::Up, true);
                    } else {
                        emu_lock.set_button(JoypadInput::Up, false);
                        emu_lock.set_button(JoypadInput::Down, false);
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

            // let AB = queue.spec().samples as f64;
            // let Ab = queue.size() as f64 / size_of::<f32>() as f64;
            // let requested = AB - Ab;

            // const DELTA: f64 = 0.0005;
            // let dyn_rate = DELTA * (1.0 + (AB - 2.0 * Ab) / AB);
            // println!(
            //     "{} {} {requested} {dyn_rate} {}",
            //     queue.spec().samples,
            //     queue.size() as f64 / size_of::<f32>() as f64,
            //     48000.0 * (1.0 + dyn_rate)
            // );
            //

            let slope = calc_slope(&w_values, w_index);
            let a = 0.05;
            let w = emu_lock.output.audiobuf.available() as f32;
            let y = prev_y * (1.0 - a) + w * a;
            prev_y = y;
            let w = y as i32;

            w_values[w_index] = w;
            rate_values[w_index] = out_freq;
            w_index = (w_index + 1) % w_values.len();

            adj_period -= 1;
            if adj_period == 0 {
                adj_period = 3;
                let diff = w - 5000;
                let dir = (diff > 0) as i32 - (diff < 0) as i32;

                let mut new_adj = 0.0;

                if dir as f32 * slope < -1.0 {
                    new_adj = slope.abs() / 4.0;
                    if new_adj > 1.0 {
                        new_adj = 1.0;
                    }
                } else if dir as f32 * slope > 0.0 || w == 0 {
                    let skew = diff.abs() as f32 / 1600.0 * 10.0;
                    new_adj = -((slope.abs() + skew) / 2.0);
                    if new_adj < -2.0 {
                        new_adj = -2.0;
                    }
                }

                new_adj *= dir as f32;
                out_freq += -new_adj;

                out_freq = out_freq.clamp(48000.0 * 0.98, 48000.0 * 1.02);
                emu_lock.set_audio_rate(out_freq as f64);
            }

            // emu_lock.set_audio_rate(48000.0 * dyn_rate);

            if frames > 2.4 {
                frames -= 2.4;
                _ = emu_lock.step_until_frame_ready();
                tex.with_lock(None, |pixels, _| {
                    pixels.copy_from_slice(emu_lock.get_video_rgba());
                })
                .unwrap();
            }
            frames += 1.0;

            // let ready = emu_lock.audio_queued();
            // let mut buf = vec![0.0; 48000 / 60 * 8];
            // emu_lock.put_audio_f32(&mut buf);
            // queue.queue_audio(&buf[..ready]).unwrap();

            // let audio = emu_lock.get_audio_f32_interp(799, (48000.0) as usize);
            // dbg!(audio.len());
            // queue.queue_audio(audio).unwrap();
        }

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        // sleep_until_fps(frame_start, frame_rate);
    }
}

fn calc_slope(w_values: &[i32; 60], w_value_index: usize) -> f32 {
    let mut valid_values = 0;
    let mut sx = 0;
    let mut sy = 0;
    let mut sxx = 0;
    let mut sxy = 0;
    let mut i = w_value_index;
    for _ in 0..w_values.len() {
        if w_values[i] != -1 {
            let k = valid_values;
            sx += k;
            sy += w_values[i];
            sxx += k * k;
            sxy += k * w_values[i];
            valid_values += 1;
        }
        i = (i + 1) % w_values.len();
    }

    let num = (valid_values * sxy - sx * sy) as f32;
    let den = (valid_values * sxx - sx * sx) as f32;
    if den == 0.0 { 0.0 } else { num / den }
}

fn sleep_until_fps(frame_start: time::Instant, frame_rate: time::Duration) {
    let frame_duration = frame_start.elapsed();
    if frame_duration < frame_rate {
        thread::sleep(frame_rate - frame_duration);
    }
}
