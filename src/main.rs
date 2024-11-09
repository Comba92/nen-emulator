use std::path::Path;

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

    let rom_path = &Path::new("roms/Super Mario Bros.nes");
    let cart = Cart::new(rom_path);

    let mut emu = Cpu::new(cart);

    'running: loop {
        emu.step_until_vblank();

        for event in sdl.events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                _ => {}
            }

            handle_input(event, &mut emu.bus.joypad);
        }

        framebuf.render_background(&emu.bus.ppu);
        framebuf.render_sprites(&emu.bus.ppu);

        texture.update(None, &framebuf.0.buffer, framebuf.0.pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();
    }
}
