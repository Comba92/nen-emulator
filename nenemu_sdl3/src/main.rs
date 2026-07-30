use std::{
    fs,
    io::{Read, Write},
    path,
    sync::{self, Arc, Mutex},
};

use nenemu_core::{emu::NesEmulator, joypad::JoypadInput};
use sdl3::{
    audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream},
    event::{Event, WindowEvent},
    gamepad::{Axis, Button},
    joystick::JoystickId,
    keyboard::Keycode,
    pixels::{Color, PixelFormat},
    render::ScaleMode,
    sys::render::SDL_LOGICAL_PRESENTATION_INTEGER_SCALE,
};
const AXIS_DEAD_ZONE: i16 = 10_000;

struct AudioHandler {
    emu: Arc<Mutex<NesEmulator>>,
}
impl AudioCallback<f32> for AudioHandler {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let mut emu_lock = self.emu.lock().unwrap();

        while emu_lock.audio_queued() < requested as usize {
            emu_lock.step();
        }

        let (right, left) = emu_lock.get_audio_f32(requested as usize);
        stream.put_data_f32(right).unwrap();
        if let Some(left) = left {
            stream.put_data_f32(left).unwrap();
        }
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

// https://github.com/giroletm/SDL2_gfx/blob/master/SDL2_framerate.c
struct FpsHandler {
    frame_count: usize,
    rate_ticks: f32,
    base_ticks: u64,
    last_ticks: u64,
}
impl FpsHandler {
    pub fn new(fps: usize) -> Self {
        let ticks = sdl3::timer::ticks().max(1);
        Self {
            frame_count: 0,
            rate_ticks: 1000.0 / fps as f32,
            base_ticks: ticks,
            last_ticks: ticks,
        }
    }

    pub fn set_framerate(&mut self, fps: usize) {
        self.frame_count = 0;
        self.rate_ticks = 1000.0 / fps as f32;
    }

    pub fn delay(&mut self) {
        self.frame_count += 1;
        let current_ticks = sdl3::timer::ticks();
        self.last_ticks = current_ticks;

        let target_ticks = self.base_ticks + (self.frame_count as f32 * self.rate_ticks) as u64;
        if current_ticks <= target_ticks {
            sdl3::timer::delay((target_ticks - current_ticks) as u32);
        } else {
            self.frame_count = 0;
            self.base_ticks = sdl3::timer::ticks();
        }
    }
}

fn main() {
    let sdl = sdl3::init().unwrap();
    let video = sdl.video().unwrap();
    let audio = sdl.audio().unwrap();
    let mut events = sdl.event_pump().unwrap();
    let controller = sdl.gamepad().unwrap();
    let mut controllers = Vec::new();

    let window = video
        .window("NesEmu", 256 * 3, 240 * 3)
        .position_centered()
        .resizable()
        .opengl()
        .build()
        .unwrap();

    let ctx = window.gl_create_context().unwrap();
    window.gl_make_current(&ctx).unwrap();
    video
        .gl_set_swap_interval(sdl3::video::SwapInterval::VSync)
        .unwrap();

    let mut canvas = window.into_canvas();

    canvas
        .set_logical_size(256, 240, SDL_LOGICAL_PRESENTATION_INTEGER_SCALE)
        .unwrap();
    let texture_creator = canvas.texture_creator();
    let mut tex = texture_creator
        .create_texture_streaming(PixelFormat::RGBA32, 256, 240)
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

    // let bios = include_bytes!("../../nenemu_core/utils/disksys.rom");
    let mut bios_path = None;
    let mut rom_path = path::PathBuf::new();

    let emu = NesEmulator::empty();
    let mut fps = FpsHandler::new(60);

    let emu = arc_mutex(emu);
    let emu_shared_clone1 = Arc::clone(&emu);

    let audiospec = AudioSpec {
        format: Some(AudioFormat::f32_sys()),
        channels: Some(1),
        freq: Some(48000),
    };

    let audiocb = audio
        .open_playback_stream(
            &audiospec,
            AudioHandler {
                emu: emu_shared_clone1,
            },
        )
        .unwrap();
    audiocb.resume().unwrap();

    'running: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::Window { win_event, .. } => match win_event {
                    WindowEvent::CloseRequested => break 'running,
                    _ => {}
                },
                Event::DropFile { filename, .. } => {
                    if filename.ends_with(".pal") {
                        let buf = fs::read(filename).unwrap();
                        _ = emu.lock().unwrap().try_set_palette(&buf);
                        continue;
                    } else if filename.contains("disksys.rom") {
                        bios_path = Some(path::PathBuf::from(&filename));
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

                            fps.set_framerate(emu_lock.frame_rate() as usize);
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
                            Keycode::_0 => emu_lock.mapper.special_input(),
                            #[cfg(feature = "savestates")]
                            Keycode::_9 => emu_lock.savestate("./save.tmp").unwrap(),
                            #[cfg(feature = "savestates")]
                            Keycode::_8 => {
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
                        Button::South => emu_lock.set_button(JoypadInput::A, true),
                        Button::West => emu_lock.set_button(JoypadInput::B, true),
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
                        Button::South => emu_lock.set_button(JoypadInput::A, false),
                        Button::West => emu_lock.set_button(JoypadInput::B, false),
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

                Event::ControllerDeviceAdded { which, .. } => match controller
                    .open(JoystickId::new(which))
                {
                    Ok(controller) => {
                        println!("Found controller: {:?}\n", controller.name());
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
            _ = emu_lock.step_until_frame_ready();
            tex.with_lock(None, |pixels, _| {
                emu_lock.put_video_rgba(pixels);
            })
            .unwrap();
        }

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        fps.delay();
    }
}
