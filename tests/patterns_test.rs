#[cfg(test)]
mod patterns_test {
    use std::path::Path;

    use nen_emulator::emu::{cart::Cart, ui::{parse_tile, FrameBuffer, Sdl2Context}};
    use sdl2::pixels::PixelFormatEnum;

    #[test]
    fn print_pattern() {
      let _tile = [
        0x41, 0xc2, 0x44, 0x48, 0x10, 0x20, 0x40, 0x80,
        0x01, 0x02, 0x04, 0x08, 0x16, 0x21, 0x42, 0x87
      ];

      let cart = Cart::new(Path::new("tests/test_roms/Pacman.nes"));

      for (i, tile) in cart.chr_rom.chunks(16).enumerate().take(100) {
        println!("Tile {i}");
        for row in 0..8 {
          let plane0 = tile[row];
          let plane1 = tile[row + 8];
  
          for bit in 0..8 {
            let bit0 = plane0 >> (7-bit) & 1;
            let bit1 = (plane1 >> (7-bit) & 1) << 1;
            let color = bit1 | bit0;
            let c = match color {
              0 => ' ',
              1 => '+',
              2 => 'z',
              3 => 'w',
              _ => unreachable!()
            };
            print!("{c}")
          }
          println!("")
        }
        println!("") 
      }
    }

    #[test]
    fn render_patterns() {
      const RENDER_WIDTH: u32 = 128*2;
      const RENDER_HEIGHT: u32 = 128;
      const SCALE: u32 = 5;

      let mut sdl = Sdl2Context::new("Patterns", RENDER_WIDTH*SCALE, RENDER_HEIGHT*SCALE);

      let mut framebuf = FrameBuffer::new(RENDER_WIDTH as usize, RENDER_HEIGHT as usize);

      let mut texture = sdl.texture_creator
      .create_texture_target(PixelFormatEnum::RGB24, framebuf.width as u32, framebuf.height as u32)
      .unwrap();

      let cart = Cart::new(Path::new("tests/test_roms/Donkey Kong.nes"));

      for (i, tile) in cart.chr_rom.chunks(16).enumerate() {
        let x = i*8 % framebuf.width;
        let y = (i*8 / framebuf.width)*8;

        let col_tile = parse_tile(tile);
        framebuf.set_tile(x, y, col_tile);
      }

      texture.update(None, &framebuf.buffer, framebuf.width*3).unwrap();
      sdl.canvas.copy(&texture, None, None).unwrap();
      sdl.canvas.present();

      'running: loop {
        for event in sdl.events.poll_iter() {
          match event {
            sdl2::event::Event::Quit { .. } => break 'running,
            _ => {}
          }
        }
      }


    }
}