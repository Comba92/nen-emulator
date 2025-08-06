use nes_emulator::{cart::Cart, emu::{Emu, SYS_COLORS}};
use sdl3::{event::Event, pixels::{PixelFormat, PixelFormatEnum}, sys::render::{SDL_RendererLogicalPresentation, SDL_LOGICAL_PRESENTATION_LETTERBOX}};

fn main() {
    let sdl = sdl3::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video.window("NesEmu", 800, 600)
        .position_centered()
        .build().unwrap();

    let mut canvas = window.into_canvas();
    canvas.set_logical_size(256, 240, SDL_RendererLogicalPresentation::LETTERBOX);
    let mut events = sdl.event_pump().unwrap();

    let texture_creator  = canvas.texture_creator();
        
    let mut tex = texture_creator
        .create_texture_streaming(None, 256, 240)
        .unwrap();

    let rom = include_bytes!("../roms/donkey kong.nes");
    let cart = Cart::new(rom).unwrap();
    let mut emu = Emu::new(cart);

    'running: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                _ => {}
            }
        }

        emu.step_until_vblank();

        let mut pixel_data = Vec::new();
        for byte in emu.framebuf {
            let color = &SYS_COLORS[byte as usize];
            pixel_data.push(color.0);
            pixel_data.push(color.1);
            pixel_data.push(color.2);
            pixel_data.push(255);
        }
        
        canvas.clear();
        tex.with_lock(None, |pixels, _| {
            pixels.copy_from_slice(&pixel_data);
        }).unwrap();

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();
    }

    // println!("{:?}", emu.framebuf);
    println!("{:?}", emu.interrupts);
    println!("{:x?}", emu.mem.vram);
    println!("{:?}", emu.mem.palettes);
}
