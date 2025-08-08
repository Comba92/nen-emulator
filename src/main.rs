use nes_emulator::{cart::Cart, emu::{Emu, SYS_COLORS}, joypad::NesButtons};
use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum};

fn main() {
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video.window("NesEmu", 800, 600)
        .position_centered()
        .resizable()
        .build().unwrap();

    let timer = sdl.timer().unwrap();
    let mut canvas = window.into_canvas()
        .accelerated()
        .build().unwrap();
    canvas.set_logical_size(256, 240).unwrap();

    let mut events = sdl.event_pump().unwrap();

    let texture_creator  = canvas.texture_creator();
        
    let mut tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256, 240)
        .unwrap();

    tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

    let rom = include_bytes!("../roms/super mario.nes");
    let cart = Cart::new(rom).unwrap();
    let mut emu = Emu::new(cart);

    let mut framebuf = Vec::new();

    'running: loop {
        let frame_start = timer.ticks64();

        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::DropFile { filename, .. } => {
                    let rom = std::fs::read(&filename).unwrap();
                    let cart = Cart::new(&rom);

                    match cart {
                        Ok(res) => {
                            emu = Emu::new(res);
                        }
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

        framebuf.clear();
        for byte in emu.framebuf {
            let color = &SYS_COLORS[byte as usize];
            framebuf.push(color.0);
            framebuf.push(color.1);
            framebuf.push(color.2);
            framebuf.push(255);
        }
        
        canvas.set_draw_color(sdl2::pixels::Color::GREY);
        canvas.clear();
        tex.with_lock(None, |pixels, _| {
            pixels.copy_from_slice(&framebuf);
        }).unwrap();

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();

        let frame_duration = timer.ticks64() - frame_start;

        if frame_duration < 17 {
            timer.delay((17 - frame_duration) as u32);
        }
    }


    println!("{:x?}", emu.mem.palettes);
}
