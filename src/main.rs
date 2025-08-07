use nes_emulator::{cart::Cart, emu::{Emu, SYS_COLORS}};
use sdl2::{event::Event, pixels::PixelFormatEnum};

fn main() {
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video.window("NesEmu", 800, 600)
        .position_centered()
        .resizable()
        .build().unwrap();

    let mut canvas = window.into_canvas()
        .present_vsync()
        .accelerated()
        .build().unwrap();
    canvas.set_logical_size(256, 240).unwrap();

    let mut events = sdl.event_pump().unwrap();

    let texture_creator  = canvas.texture_creator();
        
    let mut tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256, 240)
        .unwrap();

    tex.set_scale_mode(sdl2::render::ScaleMode::Nearest);

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
        // emu.render_nametbl0();

        let mut pixel_data = Vec::new();
        for byte in emu.framebuf {
            let color = &SYS_COLORS[byte as usize];
            pixel_data.push(color.0);
            pixel_data.push(color.1);
            pixel_data.push(color.2);
            pixel_data.push(255);
        }
        
        canvas.set_draw_color(sdl2::pixels::Color::RED);
        canvas.clear();
        tex.with_lock(None, |pixels, _| {
            pixels.copy_from_slice(&pixel_data);
        }).unwrap();

        canvas.copy(&tex, None, None).unwrap();
        canvas.present();
    }

    // println!("{:?}", emu.framebuf);
    println!("{:x?}", emu.mem.vram);
    println!("{:x?}", emu.mem.palettes);
    println!("{:x?}", emu.ppu.oam.0);
}
