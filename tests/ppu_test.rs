#[cfg(test)]
mod ppu_test {
    use std::path::Path;
    #[allow(unused_imports)]
    use log::info;
    use nen_emulator::{cart::Cart, cpu::Cpu, dev::JoypadStat, ppu::Sprite, ui::{FrameBuffer, Sdl2Context, GREYSCALE_PALETTE}};
    use sdl2::{event::Event, pixels::PixelFormatEnum, keyboard::Keycode};


    #[test]
    #[ignore]
    fn print_pattern() {
      let _tile = [
        0x41, 0xc2, 0x44, 0x48, 0x10, 0x20, 0x40, 0x80,
        0x01, 0x02, 0x04, 0x08, 0x16, 0x21, 0x42, 0x87
      ];

      let cart = Cart::new(Path::new("tests/test_roms/nestest.nes"));

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
      colog::init();

      const RENDER_WIDTH: u32 = 128*2;
      const RENDER_HEIGHT: u32 = 128;
      const SCALE: u32 = 3;

      let mut sdl = Sdl2Context::new("Patterns", RENDER_WIDTH*SCALE, RENDER_HEIGHT*SCALE);

      let mut framebuf = FrameBuffer::new(RENDER_WIDTH as usize, RENDER_HEIGHT as usize);

      let mut texture = sdl.texture_creator
      .create_texture_target(PixelFormatEnum::RGB24, framebuf.width as u32, framebuf.height as u32)
      .unwrap();

      let cart = Cart::new(Path::new("tests/nestest.nes"));

      for (i, tile) in cart.chr_rom.chunks(16).enumerate() {
        let x = i*8 % framebuf.width;
        let y = (i*8 / framebuf.width)*8;

        framebuf.set_tile(x, y, tile, &GREYSCALE_PALETTE);
      }

      texture.update(None, &framebuf.buffer, framebuf.pitch()).unwrap();
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

      println!("Framebuf: {}", framebuf.buffer.len() / 3);
    }

    #[test]
    fn render_tiles() {
      const RENDER_WIDTH: f32 = 32.0;
      const RENDER_HEIGHT: f32 = 30.0;
      const SCALE: f32 = 3.0;
      
      let mut sdl = Sdl2Context::new("Background", (8.0*RENDER_WIDTH*SCALE) as u32, (8.0*RENDER_HEIGHT*SCALE) as u32);
      let mut framebuf = FrameBuffer::new(8*RENDER_WIDTH as usize, 8*RENDER_HEIGHT as usize);
      
      let mut texture = sdl.texture_creator
      .create_texture_target(PixelFormatEnum::RGB24, framebuf.width as u32, framebuf.height as u32)
      .unwrap();
    
      // let rom_path = &Path::new("tests/nestest/nestest.nes");
      let rom_path = &Path::new("tests/test_roms/Donkey Kong.nes");
      let cart = Cart::new(rom_path);
      let mut emu = Cpu::new(cart);

      'running: loop {
        emu.step_until_vblank();

        for event in sdl.events.poll_iter() {
          match event {
            Event::Quit { .. } => break 'running,
            Event::KeyDown { keycode, .. } => {
              if let Some(keycode) = keycode {
                match keycode {
                  Keycode::Z => emu.bus.joypad.button.insert(JoypadStat::A),
                  Keycode::X => emu.bus.joypad.button.insert(JoypadStat::B),
                  Keycode::UP => emu.bus.joypad.button.insert(JoypadStat::UP),
                  Keycode::DOWN => emu.bus.joypad.button.insert(JoypadStat::DOWN),
                  Keycode::LEFT => emu.bus.joypad.button.insert(JoypadStat::LEFT),
                  Keycode::RIGHT => emu.bus.joypad.button.insert(JoypadStat::RIGHT),
                  Keycode::N => emu.bus.joypad.button.insert(JoypadStat::SELECT),
                  Keycode::M => emu.bus.joypad.button.insert(JoypadStat::START),
                  _ => {}
                }
              }
            }
            Event::KeyUp { keycode, .. } => {
              if let Some(keycode) = keycode {
                match keycode {
                  Keycode::Z => emu.bus.joypad.button.remove(JoypadStat::A),
                  Keycode::X => emu.bus.joypad.button.remove(JoypadStat::B),
                  Keycode::UP => emu.bus.joypad.button.remove(JoypadStat::UP),
                  Keycode::DOWN => emu.bus.joypad.button.remove(JoypadStat::DOWN),
                  Keycode::LEFT => emu.bus.joypad.button.remove(JoypadStat::LEFT),
                  Keycode::RIGHT => emu.bus.joypad.button.remove(JoypadStat::RIGHT),
                  Keycode::N => emu.bus.joypad.button.remove(JoypadStat::SELECT),
                  Keycode::M => emu.bus.joypad.button.remove(JoypadStat::START),
                  _ => {}
                }
              }
            }
            _ => {}
          }
        }
        
        let bg_ptrntbl = emu.bus.ppu.ctrl.bg_ptrntbl_addr() as usize;
        for i in 0..32*30 {
          let tile_idx = emu.bus.ppu.vram[i];
          let x = i as u32 % RENDER_WIDTH as u32;
          let y = i as u32 / RENDER_WIDTH as u32;
          let tile_start = bg_ptrntbl + (tile_idx as usize) * 16;
          let tile = &emu.bus.cart.chr_rom[tile_start..tile_start+16];

          let attribute_idx = (y/4 * 8) + (x/4);
          // need to do mirroring here
          let attribute = emu.bus.ppu.vram[0x3C0 + attribute_idx as usize];
          let palette_id = match (x % 4, y % 4) {
            (0..2, 0..2) => (attribute & 0b0000_0011) >> 0 & 0b11,
            (2..4, 0..2) => (attribute & 0b0000_1100) >> 2 & 0b11,
            (0..2, 2..4) => (attribute & 0b0011_0000) >> 4 & 0b11,
            (2..4, 2..4) => (attribute & 0b1100_0000) >> 6 & 0b11,
            _ => unreachable!("mod 2 should always give 0 and 1"),
          } as usize * 4;

          let palette = &emu.bus.ppu.palettes[palette_id..palette_id+4];
          framebuf.set_tile(8*x as usize, 8*y as usize, tile, palette);
        }

        let spr_ptrntbl = emu.bus.ppu.ctrl.spr_ptrntbl_addr();
        for i in (0..256).step_by(4) {
          let tile_idx = emu.bus.ppu.oam[i + 1];
          let attributes = emu.bus.ppu.oam[i + 2];
          let x = emu.bus.ppu.oam[i + 3] as usize;
          let y = emu.bus.ppu.oam[i] as usize + 1;

          let sprite = Sprite::from_bytes(&emu.bus.ppu.oam[i..i+4]);
          if x >= 8*RENDER_WIDTH as usize - 8 || y >= 8*RENDER_HEIGHT as usize - 8 { continue; }

          let tile_start = (spr_ptrntbl as usize) + (tile_idx as usize) * 16;
          let tile = &emu.bus.cart.chr_rom[tile_start..tile_start+16];
          
          let palette_id = 16 + (attributes & 0b11) as usize * 4;
          let palette = &emu.bus.ppu.palettes[palette_id..palette_id+4];
          framebuf.set_tile(x as usize, y as usize, tile, palette);
        }


        texture.update(None, &framebuf.buffer, framebuf.pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();
      }

      println!("PALETTES {:?}", emu.bus.ppu.palettes);
      println!("OAM {:?}", emu.bus.ppu.oam);

      // println!("VRAM {:?}", &emu.bus.ppu.vram);
      println!("{:?} {:?} {:?}", emu, emu.bus.ppu, emu.bus.cart.header);
    }

}