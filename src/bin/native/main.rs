use std::{env::args, path::PathBuf};
use nen_emulator::{nes::Nes, cart::Cart, frame::{SCREEN_HEIGHT, SCREEN_WIDTH}};
use sdl2::{audio::{self, AudioCallback, AudioSpecDesired, AudioStatus}, event::Event, pixels::PixelFormatEnum};
use std::{time::{Duration, Instant}, sync::mpsc::{self, Receiver}};
mod sdl2ctx;
use sdl2ctx::{handle_input, Sdl2Context};

fn main() {
    const SCALE: f32 = 3.5;
    const WINDOW_WIDTH:  u32  = (SCALE * SCREEN_WIDTH  as f32* 8.0) as u32;
    const WINDOW_HEIGHT: u32  = (SCALE * SCREEN_HEIGHT as f32* 8.0) as u32;
    let ms_frame: Duration = Duration::from_secs_f64(1.0 / 60.0988);

    let mut sdl = Sdl2Context
        ::new("NenEmulator", WINDOW_WIDTH, WINDOW_HEIGHT)
        .unwrap();
    
    // Keep aspect ratio
    sdl.canvas.set_logical_size(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32).unwrap();

    let filename = args().nth(1);
    let rom_path = if let Some(filename) = filename {
        PathBuf::from(filename)
    } else { PathBuf::from("") };

    let mut emu = Nes::empty();
    if rom_path.exists() {
        let cart = Cart::from_file(&rom_path);
        if let Ok(cart) = cart {
            let rom_name =  rom_path.file_name().unwrap().to_str().unwrap_or("NenEmulator");
            sdl.canvas.window_mut().set_title(rom_name).expect("Couldn't rename window title");
            emu = Nes::with_cart(cart);
            println!("{:#?}\n", emu.get_cart());
        }
    }

    let mut texture = sdl.texture_creator.create_texture_target(
        PixelFormatEnum::RGBA32, emu.get_screen().width as u32, emu.get_screen().height as u32
    ).unwrap();

    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1),
        samples: None,
    };

    let audio_dev = sdl.audio_subsystem
        .open_queue::<i16, _>(None, &desired_spec).unwrap();

    let mut audio_buf = Vec::new();
    audio_dev.resume();

    'running: loop {
        let ms_since_start = Instant::now();

        while emu.get_ppu().vblank_started.take().is_none() {
            if emu.is_paused { break; }
            let sample = emu.step_until_sample();
            audio_buf.push(sample);
        }

        if audio_dev.size() < 2096 {
            while emu.get_ppu().vblank_started.take().is_none() {
                if emu.is_paused { break; }
                let sample = emu.step_until_sample();
                audio_buf.push(sample);
            }
        }

        audio_dev.queue_audio(&audio_buf);
        audio_buf.clear();

        for event in sdl.events.poll_iter() {
            handle_input(&sdl.keymaps, &event, &mut emu);

            match event {
                Event::Quit { .. } => {
                    break 'running;
                }
                Event::DropFile { filename, .. } => {

                    let rom_path = &PathBuf::from(filename);
                    let rom_result = Cart::from_file(&rom_path);

                    match rom_result {
                        Ok(cart) => {
                            let rom_name =  rom_path.file_name().unwrap().to_str().unwrap_or("NenEmulator");
                            sdl.canvas.window_mut().set_title(rom_name).expect("Couldn't rename window title");
                            emu.load_cart(cart);
                            println!("{:#?}\n", emu.get_cart());
                        }
                        Err(msg) => eprintln!("Couldn't load the rom: {msg}\n"),
                    };
                }
                Event::ControllerDeviceAdded { which , .. } => {
                    match sdl.controller_subsystem.open(which) {
                        Ok(controller) => {
                            eprintln!("Found controller: {}\n", controller.name());
                            sdl.controllers.push(controller);
                        }
                        Err(_) => eprintln!("A controller was connected, but I couldn't initialize it\n")
                    }
                }
                _ => {}
            }
        }

        sdl.canvas.clear();
        texture.update(None, &emu.get_screen().buffer, emu.get_screen().pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();

        let ms_elapsed = Instant::now() - ms_since_start;
        if ms_frame > ms_elapsed {
            std::thread::sleep(ms_frame - ms_elapsed);
        }
    }
}
