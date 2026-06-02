use std::{
    fs,
    io::{BufReader, BufWriter, Read, Write},
    path,
    sync::mpsc::{self, Receiver},
};

use nes_emulator::{emu::Emu, joypad::JoypadBtn};
use sdl2::{
    audio::AudioCallback,
    event::{Event, WindowEvent},
    keyboard::Keycode,
    pixels::PixelFormatEnum,
};

struct AudioHandler(Receiver<i16>);
impl AudioCallback for AudioHandler {
    type Channel = i16;

    fn callback(&mut self, buf: &mut [Self::Channel]) {
        for sample in buf {
            *sample = self.0.recv().unwrap();
        }
    }
}

fn main() {
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let audio = sdl.audio().unwrap();

    let audiospec = sdl2::audio::AudioSpecDesired {
        channels: Some(1),
        freq: Some(48000),
        samples: Some(1024),
    };

    let (send, recv) = mpsc::channel();

    let audiodev = audio.open_queue::<i16, _>(None, &audiospec).unwrap();
    let audiocb = audio
        .open_playback(None, &audiospec, |_| AudioHandler(recv))
        .unwrap();

    // audiocb.resume();
    audiodev.resume();
    // println!("{:?}", audiodev.spec());

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
    let rom = include_bytes!("../../roms/super mario.nes");
    let mut emu = Emu::load_rom_from_bytes(rom, Some(bios)).unwrap();
    let mut rom_filename = "../roms/super mario.nes".to_string();

    let mut frame_rate = (1.0 / emu.frame_rate() * 1000.0).round() as u64;

    let mut avg_missed = 0;
    let mut frames_missed = 0;
    let mut frames_count = 0;
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
                    if filename.ends_with(".pal") {
                        let buf = fs::read(filename).unwrap();
                        emu.load_palette(&buf);
                        continue;
                    }

                    let new_emu = Emu::load_rom_from_file(&filename, Some(bios));
                    match new_emu {
                        Ok(res) => {
                            // save current game battery
                            if let Some(sram) = emu.save_battery() {
                                let mut save_path = path::PathBuf::from(&rom_filename);
                                save_path.set_extension("sram");

                                let file = fs::File::create(&save_path).unwrap();
                                let mut writer = BufWriter::new(file);
                                writer.write_all(sram).unwrap();
                                println!("Battery saved to {save_path:?}");
                            }

                            rom_filename = filename;
                            emu = res;
                            println!("{:?}", emu.header());

                            // load current game battery if any
                            let mut load_path = path::PathBuf::from(&rom_filename);
                            load_path.set_extension("sram");
                            if let Ok(file) = fs::File::open(&load_path) {
                                let mut buf = Vec::new();
                                let mut reader = BufReader::new(file);
                                reader.read_to_end(&mut buf).unwrap();
                                let res = emu.load_battery(&buf);
                                match res {
                                    Err(e) => eprintln!("{e}"),
                                    _ => println!("Battery loaded from {load_path:?}"),
                                }
                            }

                            frame_rate = (1.0 / emu.frame_rate() * 1000.0).round() as u64;
                            audiodev.clear();
                        }
                        Err(e) => eprintln!("{e}"),
                    }
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        match keycode {
                            Keycode::W => emu.set_button(JoypadBtn::Up, true),
                            Keycode::A => emu.set_button(JoypadBtn::Left, true),
                            Keycode::S => emu.set_button(JoypadBtn::Down, true),
                            Keycode::D => emu.set_button(JoypadBtn::Right, true),
                            Keycode::K => emu.set_button(JoypadBtn::A, true),
                            Keycode::J => emu.set_button(JoypadBtn::B, true),
                            Keycode::M => emu.set_button(JoypadBtn::Start, true),
                            Keycode::N => emu.set_button(JoypadBtn::Select, true),
                            Keycode::NUM_0 => emu.mapper.special_input(),
                            #[cfg(feature = "serde")]
                            Keycode::NUM_9 => emu.savestate("./save.tmp").unwrap(),
                            #[cfg(feature = "serde")]
                            Keycode::NUM_8 => {
                                emu.loadstate("./save.tmp").unwrap();
                                audiodev.clear();
                            }
                            Keycode::R => emu.emu_reset(),
                            _ => {}
                        }
                    }
                }

                Event::KeyUp { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        match keycode {
                            Keycode::W => emu.set_button(JoypadBtn::Up, false),
                            Keycode::A => emu.set_button(JoypadBtn::Left, false),
                            Keycode::S => emu.set_button(JoypadBtn::Down, false),
                            Keycode::D => emu.set_button(JoypadBtn::Right, false),
                            Keycode::K => emu.set_button(JoypadBtn::A, false),
                            Keycode::J => emu.set_button(JoypadBtn::B, false),
                            Keycode::M => emu.set_button(JoypadBtn::Start, false),
                            Keycode::N => emu.set_button(JoypadBtn::Select, false),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        emu.step_until_vblank();

        // for sample in emu.get_audio() {
        //     send.send(*sample).unwrap()
        // }

        audiodev.queue_audio(emu.get_audio()).unwrap();

        while audiodev.size() / 2 < audiodev.spec().samples as u32 {
            // run for another frame

            emu.step_until_vblank();
            audiodev.queue_audio(emu.get_audio()).unwrap();

            frames_missed += 1;
        }

        frames_count += 1;
        if frames_count % 60 == 0 {
            // println!("Missed this second: {frames_missed}");
            avg_missed += frames_missed / 60;
            frames_missed = 0;
        }

        canvas.set_draw_color(sdl2::pixels::Color::GREY);
        canvas.clear();

        tex.with_lock(None, |pixels, _| {
            emu.get_video_rgba(pixels);
        })
        .unwrap();

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
