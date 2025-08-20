use nes_emulator::{emu::{Emu, DEFAULT_PALETTE}, joypad::NesButtons};
use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum};

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
        samples: Some(2048),
    };
    let audiodev = audio.open_queue(None, &audiospec).unwrap();
    audiodev.resume();

    let mut events = sdl.event_pump().unwrap();

    let texture_creator  = canvas.texture_creator();
        
    let mut tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256, 240)
        .unwrap();

    tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

    let mut emu = Emu::new(include_bytes!("../roms/prince of persia.nes")).unwrap();

    let mut framebuf = [0; 256 * 240 * 4];

    let mut avg_missed = 0;
    let mut frames_missed = 0;
    let mut frames_count = 0;
    'running: loop {
        let frame_start = timer.ticks64();

        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::DropFile { filename, .. } => {
                    let rom = std::fs::read(&filename).unwrap();
                    let new_emu = Emu::new(&rom);

                    match new_emu {
                        Ok(res) => emu = res,
                        Err (e) => eprintln!("{e}"),
                    }
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        match keycode {
                            Keycode::W => emu.joypad.set_button(NesButtons::Up, true),
                            Keycode::A => emu.joypad.set_button(NesButtons::Left, true),
                            Keycode::S => emu.joypad.set_button(NesButtons::Down, true),
                            Keycode::D => emu.joypad.set_button(NesButtons::Right, true),
                            Keycode::J => emu.joypad.set_button(NesButtons::A, true),
                            Keycode::K => emu.joypad.set_button(NesButtons::B, true),
                            Keycode::M => emu.joypad.set_button(NesButtons::Start, true),
                            Keycode::N => emu.joypad.set_button(NesButtons::Select, true),
                            _ => {}
                        }
                    }
                }

                Event::KeyUp { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        match keycode {
                            Keycode::W => emu.joypad.set_button(NesButtons::Up, false),
                            Keycode::A => emu.joypad.set_button(NesButtons::Left, false),
                            Keycode::S => emu.joypad.set_button(NesButtons::Down, false),
                            Keycode::D => emu.joypad.set_button(NesButtons::Right, false),
                            Keycode::J => emu.joypad.set_button(NesButtons::A, false),
                            Keycode::K => emu.joypad.set_button(NesButtons::B, false),
                            Keycode::M => emu.joypad.set_button(NesButtons::Start, false),
                            Keycode::N => emu.joypad.set_button(NesButtons::Select, false),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        emu.step_until_vblank();
        audiodev.queue_audio(emu.get_audio()).unwrap();

        while audiodev.size() < audiodev.spec().samples as u32 * 4 {
            // run for another frame
            // println!("Running another frame for filling the audio queue... {}", audiodev.size());

            emu.step_until_vblank();
            audiodev.queue_audio(emu.get_audio()).unwrap();
            // println!("Are we filled? {}", audiodev.size());

            // frames_missed += 1;
        }

        // frames_count += 1;
        // if frames_count % 60 == 0 {
        //     println!("Missed this second: {frames_missed}");
        //     avg_missed += frames_missed / 60;
        //     frames_missed = 0;
        // }

        
        emu.get_video_rgba(&mut framebuf);
        canvas.set_draw_color(sdl2::pixels::Color::GREY);
        canvas.clear();
        
        tex.with_lock(None, |pixels, _| {
            pixels.copy_from_slice(&framebuf);
        }).unwrap();

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        let frame_duration = timer.ticks64() - frame_start;

        if frame_duration < 16 {
            timer.delay((16 - frame_duration) as u32);
        }
    }

    println!("{:02X?}", emu.mem.palettes);
}
