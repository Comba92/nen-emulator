#![allow(dead_code)]
use sdl2::event::Event;

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
    'running: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                _ => {}
            }
        }
    }
}
