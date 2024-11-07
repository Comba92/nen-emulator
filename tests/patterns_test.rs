#[cfg(test)]
mod patterns_test {
    use std::path::Path;
    #[allow(unused_imports)]
    use log::info;
    use nen_emulator::{cart::Cart, cpu::Cpu, ui::{parse_tile, FrameBuffer, Sdl2Context}};
    use sdl2::{event::Event, pixels::PixelFormatEnum};


    #[test]
    #[ignore]
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
      colog::init();

      const RENDER_WIDTH: u32 = 128*2;
      const RENDER_HEIGHT: u32 = 128;
      const SCALE: u32 = 3;

      let mut sdl = Sdl2Context::new("Patterns", RENDER_WIDTH*SCALE, RENDER_HEIGHT*SCALE);

      let mut framebuf = FrameBuffer::new(RENDER_WIDTH as usize, RENDER_HEIGHT as usize);

      let mut texture = sdl.texture_creator
      .create_texture_target(PixelFormatEnum::RGB24, framebuf.width as u32, framebuf.height as u32)
      .unwrap();

      let cart = Cart::new(Path::new("tests/Balloon Fight.nes"));

      for (i, tile) in cart.chr_rom.chunks(16).enumerate() {
        let x = i*8 % framebuf.width;
        let y = (i*8 / framebuf.width)*8;

        let col_tile = parse_tile(tile);
        framebuf.set_tile(x, y, col_tile);
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
      const RENDER_WIDTH: u32 = 32;
      const RENDER_HEIGHT: u32 = 30;
      const SCALE: u32 = 25;
      
      let mut sdl = Sdl2Context::new("Background", RENDER_WIDTH*SCALE, RENDER_HEIGHT*SCALE);
      let mut framebuf = FrameBuffer::new(8*RENDER_WIDTH as usize, 8*RENDER_HEIGHT as usize);
      
      let mut texture = sdl.texture_creator
      .create_texture_target(PixelFormatEnum::RGB24, framebuf.width as u32, framebuf.height as u32)
      .unwrap();
    
      let rom_path = &Path::new("tests/Pacman.nes");
      let cart = Cart::new(rom_path);
      let mut emu = Cpu::new(cart);

      // let mut builder = colog::basic_builder();
      // builder.filter_level(log::LevelFilter::Trace);
      // builder.init();

      // // colog::init();
      // for _ in 0..200 {
      //   emu.step();
      // }

      'running: loop {
        for _ in 0..260 {
          emu.step();
        }

        for event in sdl.events.poll_iter() {
          match event {
            Event::Quit { .. } => break 'running,
            _ => {}
          }
        }
        
        let bg_ptrntbl = emu.bus.ppu.ctrl.bg_ptrntbl_addr();
        for i in 0..32*30 {
          let tile_idx = emu.bus.ppu.vram[0x2000 + i];
          let x = i as u32 % RENDER_WIDTH;
          let y = i as u32 / RENDER_WIDTH;
          let tile_start = (bg_ptrntbl as usize) + (tile_idx as usize) * 16;
          let tile = &emu.bus.cart.chr_rom[tile_start..tile_start+16];
          framebuf.set_tile(8*x as usize, 8*y as usize, parse_tile(tile));
        }

        let spr_ptrntbl = emu.bus.ppu.ctrl.spr_ptrntbl_addr();
        for i in (0..256).step_by(4) {
          let tile_idx = emu.bus.ppu.oam[i + 1];
          let x = emu.bus.ppu.oam[i + 3] as usize;
          let y = emu.bus.ppu.oam[i] as usize;
          let tile_start = (spr_ptrntbl as usize) + (tile_idx as usize) * 16;
          let tile = &emu.bus.cart.chr_rom[tile_start..tile_start+16];
          framebuf.set_tile(8*x as usize, 8*y as usize, parse_tile(tile));
        }
        //break 'running;

        texture.update(None, &framebuf.buffer, framebuf.pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();
      }

      // println!("VRAM filled {:?} times",vram_filled);
      // println!("OAM filled {:?} times", oam_filled);

      println!("OAM {:?}", emu.bus.ppu.oam);
      println!("VRAM {:?}", &emu.bus.ppu.vram[0x2000..0x4000]);
      println!("{:?} {:?} {:?}", emu, emu.bus.ppu, emu.bus.cart.header);
    }

}