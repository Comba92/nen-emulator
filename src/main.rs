use std::{env::args, path::PathBuf};

use nen_emulator::{cart::Cart, cpu::Cpu, renderer::{handle_input, NesScreen, Sdl2Context, SCREEN_HEIGHT, SCREEN_WIDTH}};
use sdl2::{event::Event, pixels::PixelFormatEnum};

fn main() {
    const SCALE: f32 = 4.0;
    const WINDOW_WIDTH:  u32  = (SCALE * SCREEN_WIDTH  as f32* 8.0) as u32;
    const WINDOW_HEIGHT: u32  = (SCALE * SCREEN_HEIGHT as f32* 8.0) as u32;

    let mut sdl = Sdl2Context::new("NenEmulator", WINDOW_WIDTH, WINDOW_HEIGHT);
    
    let mut framebuf = NesScreen::new();
    let mut texture = sdl.texture_creator.create_texture_target(
        PixelFormatEnum::RGB24, framebuf.0.width as u32, framebuf.0.height as u32
    ).unwrap();

    let filename = args().nth(1);
    let rom_path = if let Some(filename) = filename {
        PathBuf::from(filename)
    } else { PathBuf::from("roms/Donkey Kong.nes") };



    let mut emu = Cpu::from_rom_path(&rom_path);

    'running: loop {
        emu.step_until_vblank();

        for event in sdl.events.poll_iter() {
            handle_input(&event, &mut emu.bus.joypad);

            match event {
                Event::Quit { .. } => break 'running,
                Event::DropFile { filename, .. } => {
                    emu = Cpu::from_rom_path(&PathBuf::from(filename))
                }
                _ => {}
            }
        }

        framebuf.render_background(&emu.bus.ppu);
        framebuf.render_sprites(&emu.bus.ppu);

        texture.update(None, &framebuf.0.buffer, framebuf.0.pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();
    }
}
