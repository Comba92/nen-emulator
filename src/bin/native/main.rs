use std::{env::args, path::PathBuf};
use nen_emulator::{cart::Cart, emu::Emu, render::{SCREEN_HEIGHT, SCREEN_WIDTH}};
use sdl2::{pixels::PixelFormatEnum, event::Event};
use sdl2ctx::{handle_input, Sdl2Context};

pub mod sdl2ctx;

fn main() {
    const SCALE: f32 = 3.5;
    const WINDOW_WIDTH:  u32  = (SCALE * SCREEN_WIDTH  as f32* 8.0) as u32;
    const WINDOW_HEIGHT: u32  = (SCALE * SCREEN_HEIGHT as f32* 8.0) as u32;

    let mut sdl = Sdl2Context::new("NenEmulator", WINDOW_WIDTH, WINDOW_HEIGHT);
    
    // Keep aspect ratio
    sdl.canvas.set_logical_size(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32).unwrap();

    let filename = args().nth(1);
    let rom_path = if let Some(filename) = filename {
        PathBuf::from(filename)
    } else { PathBuf::from("") };

    let mut emu = Emu::empty();
    if rom_path.exists() {
        let cart = Cart::from_file(&rom_path);
        if let Ok(cart) = cart {
            emu = Emu::with_cart(cart);
        }
    }

    let mut texture = sdl.texture_creator.create_texture_target(
        PixelFormatEnum::RGBA32, emu.get_screen().width as u32, emu.get_screen().height as u32
    ).unwrap();

    'running: loop {
        let ticks_since_start = sdl.timer.performance_counter();
        emu.step_until_vblank();

        for event in sdl.events.poll_iter() {
            handle_input(&sdl.keymaps, &event, &mut emu);

            match event {
                Event::Quit { .. } => break 'running,
                Event::DropFile { filename, .. } => {
                    let rom_path = &PathBuf::from(filename);
                    let rom_result = Cart::from_file(&rom_path);

                    match rom_result {
                        Ok(cart) => emu = Emu::with_cart(cart),
                        Err(msg) => eprintln!("Couldn't load the rom: {msg}"),
                    }
                }
                Event::ControllerDeviceAdded { which , .. } => {
                    match sdl.controller_subsystem.open(which) {
                        Ok(controller) => {
                            eprintln!("Found controller: {}", controller.name());
                            sdl.controllers.push(controller);
                        }
                        Err(_) => eprintln!("A controller was connected, but I couldn't initialize it")
                    }
                }
                _ => {}
            }
        }

        sdl.canvas.clear();
        texture.update(None, &emu.get_screen().buffer, emu.get_screen().pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();

        // TODO: fix fps capping
        let elapsed_ms = (sdl.timer.performance_counter() - ticks_since_start) as f64 
            / sdl.timer.performance_frequency() as f64
            * 1000.0;
        sdl.timer.delay(((1.0/59.94 * 1000.0) - elapsed_ms) as u32);
    }
}
