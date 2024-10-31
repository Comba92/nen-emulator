
static GAME_CODE: [u8; 309] = [
    0x20, 0x06, 0x06, 0x20, 0x38, 0x06, 0x20, 0x0d, 0x06, 0x20, 0x2a, 0x06, 0x60, 0xa9, 0x02, 0x85,
    0x02, 0xa9, 0x04, 0x85, 0x03, 0xa9, 0x11, 0x85, 0x10, 0xa9, 0x10, 0x85, 0x12, 0xa9, 0x0f, 0x85,
    0x14, 0xa9, 0x04, 0x85, 0x11, 0x85, 0x13, 0x85, 0x15, 0x60, 0xa5, 0xfe, 0x85, 0x00, 0xa5, 0xfe,
    0x29, 0x03, 0x18, 0x69, 0x02, 0x85, 0x01, 0x60, 0x20, 0x4d, 0x06, 0x20, 0x8d, 0x06, 0x20, 0xc3,
    0x06, 0x20, 0x19, 0x07, 0x20, 0x20, 0x07, 0x20, 0x2d, 0x07, 0x4c, 0x38, 0x06, 0xa5, 0xff, 0xc9,
    0x77, 0xf0, 0x0d, 0xc9, 0x64, 0xf0, 0x14, 0xc9, 0x73, 0xf0, 0x1b, 0xc9, 0x61, 0xf0, 0x22, 0x60,
    0xa9, 0x04, 0x24, 0x02, 0xd0, 0x26, 0xa9, 0x01, 0x85, 0x02, 0x60, 0xa9, 0x08, 0x24, 0x02, 0xd0,
    0x1b, 0xa9, 0x02, 0x85, 0x02, 0x60, 0xa9, 0x01, 0x24, 0x02, 0xd0, 0x10, 0xa9, 0x04, 0x85, 0x02,
    0x60, 0xa9, 0x02, 0x24, 0x02, 0xd0, 0x05, 0xa9, 0x08, 0x85, 0x02, 0x60, 0x60, 0x20, 0x94, 0x06,
    0x20, 0xa8, 0x06, 0x60, 0xa5, 0x00, 0xc5, 0x10, 0xd0, 0x0d, 0xa5, 0x01, 0xc5, 0x11, 0xd0, 0x07,
    0xe6, 0x03, 0xe6, 0x03, 0x20, 0x2a, 0x06, 0x60, 0xa2, 0x02, 0xb5, 0x10, 0xc5, 0x10, 0xd0, 0x06,
    0xb5, 0x11, 0xc5, 0x11, 0xf0, 0x09, 0xe8, 0xe8, 0xe4, 0x03, 0xf0, 0x06, 0x4c, 0xaa, 0x06, 0x4c,
    0x35, 0x07, 0x60, 0xa6, 0x03, 0xca, 0x8a, 0xb5, 0x10, 0x95, 0x12, 0xca, 0x10, 0xf9, 0xa5, 0x02,
    0x4a, 0xb0, 0x09, 0x4a, 0xb0, 0x19, 0x4a, 0xb0, 0x1f, 0x4a, 0xb0, 0x2f, 0xa5, 0x10, 0x38, 0xe9,
    0x20, 0x85, 0x10, 0x90, 0x01, 0x60, 0xc6, 0x11, 0xa9, 0x01, 0xc5, 0x11, 0xf0, 0x28, 0x60, 0xe6,
    0x10, 0xa9, 0x1f, 0x24, 0x10, 0xf0, 0x1f, 0x60, 0xa5, 0x10, 0x18, 0x69, 0x20, 0x85, 0x10, 0xb0,
    0x01, 0x60, 0xe6, 0x11, 0xa9, 0x06, 0xc5, 0x11, 0xf0, 0x0c, 0x60, 0xc6, 0x10, 0xa5, 0x10, 0x29,
    0x1f, 0xc9, 0x1f, 0xf0, 0x01, 0x60, 0x4c, 0x35, 0x07, 0xa0, 0x00, 0xa5, 0xfe, 0x91, 0x00, 0x60,
    0xa6, 0x03, 0xa9, 0x00, 0x81, 0x10, 0xa2, 0x00, 0xa9, 0x01, 0x81, 0x10, 0x60, 0xa2, 0x00, 0xea,
    0xea, 0xca, 0xd0, 0xfb, 0x60
];

#[cfg(test)]
mod snake_test {
    use std::time::Duration;

    use nen_emulator::emu::cpu::Cpu;
    use rand::Rng;
    use sdl2::{event::Event, keyboard::Keycode, pixels::{Color, PixelFormatEnum}};

    use super::GAME_CODE;

    const PIXEL_SCALE: f32 = 25.0;

  #[test]
  fn run_snake() {
    let ctx = sdl2::init().expect("Couldn't initialize SDL2");
    let video= ctx.video().expect("Couldn't initialize video subsystem");
    let mut canvas = video.window("Nen-Emulator", (32.0 * PIXEL_SCALE) as u32, (32.0 * PIXEL_SCALE) as u32)
        .position_centered()
        .build().expect("Couldn't initialize window")
        .into_canvas()
        .accelerated().present_vsync()
        .build().expect("Couldn't initialize drawing canvas");

    let mut events = ctx.event_pump().expect("Couldn't get the event pump");
    canvas.set_scale(PIXEL_SCALE, PIXEL_SCALE).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture_target(PixelFormatEnum::RGB24, 32, 32)
        .unwrap();

    let mut cpu = Cpu::new();
    cpu.write_data(0x600, &GAME_CODE);
    cpu.pc = 0x600;

    let mut framebuffer = [0u8; 32*32*3];
    let mut rng = rand::thread_rng();

    cpu.interpret_with_callback(|cpu| {
        for event in events.poll_iter() {
          match event {
            Event::Quit { .. } => return true,
            Event::KeyDown { keycode: Some(Keycode::W), .. } => {
              cpu.mem_set(0xFF, 0x77);
            }
            Event::KeyDown { keycode: Some(Keycode::S), .. } => {
              cpu.mem_set(0xFF, 0x73);
            }
            Event::KeyDown { keycode: Some(Keycode::A), .. } => {
              cpu.mem_set(0xFF, 0x61);
            }
            Event::KeyDown { keycode: Some(Keycode::D), .. } => {
              cpu.mem_set(0xFF, 0x64);
            }
            _ => {}
        }
      }

      cpu.mem_set(0xfe, rng.gen_range(2..16));

      let mut to_update = false;
      for i in 0..(0x600-0x200) {
        let color_idx = cpu.mem_fetch(0x200 + i);
        let color = match color_idx {
          0 => Color::BLACK,
          1 => Color::WHITE,
          2 | 9 => Color::GREY,
          3 | 10 => Color::RED,
          4 | 11 => Color::GREEN,
          5 | 12 => Color::BLUE,
          6 | 13 => Color::MAGENTA,
          7 | 14 => Color::YELLOW,
          8 | 15 => Color::CYAN,
          _ => Color::BLACK
        };

        let (r, g, b) = color.rgb();

        if framebuffer[i as usize * 3 + 0] != r 
        || framebuffer[i as usize * 3 + 1] != g 
        || framebuffer[i as usize * 3 + 2] != b {
          framebuffer[i as usize * 3 + 0] = r;
          framebuffer[i as usize * 3 + 1] = g;
          framebuffer[i as usize * 3 + 2] = b;
          to_update = true;
        }
      }

      if to_update {
        texture.update(None, &framebuffer, 32*3).unwrap();
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
      }

      std::thread::sleep(Duration::new(0, 10_000));
      false
    });

    println!("{:?}", cpu);
  }
  
}