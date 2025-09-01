use std::io::{BufReader, Read, Seek};

use nes_emulator::{emu::Emu, joypad::Button};
use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum};

fn load_rom(path: &str) -> Result<Emu, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    let mut file = std::fs::File::open(path)?;

    let _ = zip::read::ZipArchive::new(BufReader::new(&file))
        .map_err(|e| e.into())
        .and_then(|mut archive| {
            archive.by_index(0)
            .map_err(|e| e.into())
            .and_then(|mut zip| {
                zip.read_to_end(&mut bytes)
            })
        }).or_else(|_| {
            file.rewind().and_then(|_| BufReader::new(&file).read_to_end(&mut bytes))
        })?;

    Emu::new(&bytes).map_err(|e| e.into())
}

fn main() {
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video.window("NesEmu", 256 * 3, 240 * 3)
        .position_centered()
        .resizable()
        .build().unwrap();
    let audio = sdl.audio().unwrap();

    let timer = sdl.timer().unwrap();
    let mut canvas = window.into_canvas()
        .accelerated()
        .build().unwrap();
    canvas.set_logical_size(256, 240).unwrap();

    let audiospec = sdl2::audio::AudioSpecDesired {
        channels: Some(1),
        freq: Some(48000),
        samples: None,
    };
    let audiodev = audio.open_queue(None, &audiospec).unwrap();
    audiodev.resume();
    println!("{:?}", audiodev.spec());

    let mut events = sdl.event_pump().unwrap();

    let texture_creator  = canvas.texture_creator();
        
    let mut tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256, 240)
        .unwrap();

    tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

    let mut emu = Emu::new(include_bytes!("../roms/super mario.nes")).unwrap();

    let mut framebuf = [0; 256 * 240 * 4];
    let mut frame_rate = 1 + (1.0 / emu.frame_rate() * 1000.0) as u64;
    println!("{frame_rate}");


    let mut avg_missed = 0;
    let mut frames_missed = 0;
    let mut frames_count = 0;
    'running: loop {
        let frame_start = timer.ticks64();

        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::DropFile { filename, .. } => {
                    let bytes = std::fs::read(&filename).unwrap();
                    
                    if filename.ends_with(".pal") {
                        emu.load_palette(&bytes);
                        continue;
                    }

                    let new_emu = load_rom(&filename);

                    match new_emu {
                        Ok(res) => {
                            emu = res;
                            frame_rate = 1 + (1.0 / emu.frame_rate() * 1000.0) as u64;
                            println!("{frame_rate}");
                            audiodev.clear();
                        },
                        Err (e) => eprintln!("{e}"),
                    }
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        match keycode {
                            Keycode::W => emu.joypad.set_button(Button::Up, true),
                            Keycode::A => emu.joypad.set_button(Button::Left, true),
                            Keycode::S => emu.joypad.set_button(Button::Down, true),
                            Keycode::D => emu.joypad.set_button(Button::Right, true),
                            Keycode::K => emu.joypad.set_button(Button::A, true),
                            Keycode::J => emu.joypad.set_button(Button::B, true),
                            Keycode::M => emu.joypad.set_button(Button::Start, true),
                            Keycode::N => emu.joypad.set_button(Button::Select, true),
                            _ => {}
                        }
                    }
                }

                Event::KeyUp { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        match keycode {
                            Keycode::W => emu.joypad.set_button(Button::Up, false),
                            Keycode::A => emu.joypad.set_button(Button::Left, false),
                            Keycode::S => emu.joypad.set_button(Button::Down, false),
                            Keycode::D => emu.joypad.set_button(Button::Right, false),
                            Keycode::K => emu.joypad.set_button(Button::A, false),
                            Keycode::J => emu.joypad.set_button(Button::B, false),
                            Keycode::M => emu.joypad.set_button(Button::Start, false),
                            Keycode::N => emu.joypad.set_button(Button::Select, false),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        emu.step_until_vblank();
        audiodev.queue_audio(emu.get_audio()).unwrap();

        while audiodev.size()/2 < audiodev.spec().samples as u32 * 2 {
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

        
        emu.get_video_rgba(&mut framebuf);
        canvas.set_draw_color(sdl2::pixels::Color::GREY);
        canvas.clear();
        
        tex.with_lock(None, |pixels, _| {
            pixels.copy_from_slice(&framebuf);
        }).unwrap();

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        let frame_duration = timer.ticks64() - frame_start;

        if frame_duration < frame_rate {
            timer.delay((frame_rate - frame_duration) as u32);
        }
    }
}
