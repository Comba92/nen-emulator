use std::{env::args, path::PathBuf};

use nen_emulator::{cart::Cart, cpu::Cpu, renderer::{handle_input, Sdl2Context}, tile::{SCREEN_HEIGHT, SCREEN_WIDTH}};
use sdl2::{event::Event, pixels::PixelFormatEnum};

fn main() {
    const SCALE: f32 = 3.5;
    const WINDOW_WIDTH:  u32  = (SCALE * SCREEN_WIDTH  as f32* 8.0) as u32;
    const WINDOW_HEIGHT: u32  = (SCALE * SCREEN_HEIGHT as f32* 8.0) as u32;

    let mut sdl = Sdl2Context::new("NenEmulator", WINDOW_WIDTH, WINDOW_HEIGHT);
    // Keep aspect ratio
    sdl.canvas.set_logical_size(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32).unwrap();

    let filename = args().nth(1);
    let _rom_path = if let Some(filename) = filename {
        PathBuf::from(filename)
    } else { PathBuf::from("roms/Donkey Kong.nes") };

    // let mut emu = Cpu::from_rom_path(&rom_path);
    let mut emu = Cpu::new(Cart::empty());

    let mut texture = sdl.texture_creator.create_texture_target(
        PixelFormatEnum::RGB24, emu.get_screen().width as u32, emu.get_screen().height as u32
    ).unwrap();


    'running: loop {
        let ticks_since_start = sdl.timer.performance_counter();
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

        sdl.canvas.clear();
        texture.update(None, &emu.get_screen().buffer, emu.get_screen().pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();

        let elapsed_ms = (sdl.timer.performance_counter() - ticks_since_start) as f64 
            / sdl.timer.performance_frequency() as f64
            * 1000.0;
        sdl.timer.delay(((1.0/59.94 * 1000.0) - elapsed_ms) as u32);
    }

    println!("{:?}", emu.bus.ppu.palettes);
}
