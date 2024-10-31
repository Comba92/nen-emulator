#![allow(dead_code)]
use sdl2::{event::Event, pixels::PixelFormatEnum};

pub fn show() {
    let ctx = sdl2::init().expect("Couldn't initialize SDL2");
    let video= ctx.video().expect("Couldn't initialize video subsystem");
    let mut canvas = video.window("Nen-Emulator", 800, 600)
        .position_centered()
        .build().expect("Couldn't initialize window")
        .into_canvas()
        .accelerated().present_vsync()
        .build().expect("Couldn't initialize drawing canvas");

    let mut events = ctx.event_pump().expect("Couldn't get the event pump");
    canvas.set_scale(10.0, 10.0).unwrap();

    let creator = canvas.texture_creator();
    let mut _texture = creator
        .create_texture_target(PixelFormatEnum::RGB24, 32, 32);

    'running: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                _ => {}
            }
        }
    }
}
