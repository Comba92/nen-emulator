use crate::{bus::Bus, dma::Dma};

pub struct MemMapping {
  pub cpu_reads:  [fn(&mut Bus, u16) -> u8; 8],
  pub cpu_writes: [fn(&mut Bus, u16, u8); 8],
  pub ppu_reads:  [fn(&mut Bus, u16) -> u8; 16],
  pub ppu_writes: [fn(&mut Bus, u16, u8); 16],
}

pub fn prg_read(bus: &mut Bus, addr: u16) -> u8 {
  bus.prg[bus.mapper.prg_translate(&mut bus.cfg, addr)]
}
pub fn prg_write(bus: &mut Bus, addr: u16, val: u8) {
  bus.mapper.prg_write(&mut bus.cfg, addr as usize, val);
}
pub fn sram_read(bus: &mut Bus, addr: u16) -> u8 {
  bus.sram[bus.cfg.sram.translate(addr as usize)]
}
pub fn sram_write(bus: &mut Bus, addr: u16, val: u8) {
  bus.sram[bus.cfg.sram.translate(addr as usize)] = val;
}
// pub fn sram_read(bus: &mut Bus, addr: u16) -> u8 {
//   bus.sram[bus.mapper.sram_translate(&mut bus.cfg, addr)]
// }
// pub fn sram_write(bus: &mut Bus, addr: u16, val: u8) {
//   bus.sram[bus.mapper.sram_translate(&mut bus.cfg, addr)] = val;
// }
pub fn chr_read (bus: &mut Bus, addr: u16) -> u8 {
  bus.chr[bus.mapper.chr_translate(&mut bus.cfg, addr)]
}
pub fn chr_write (bus: &mut Bus, addr: u16, val: u8) {
  bus.chr[bus.mapper.chr_translate(&mut bus.cfg, addr)] = val;
}
pub fn vram_read(bus: &mut Bus, addr: u16) -> u8 {
  bus.vram[bus.mapper.vram_translate(&mut bus.cfg, addr)]
}
pub fn vram_write(bus: &mut Bus, addr: u16, val: u8) {
  bus.vram[bus.mapper.vram_translate(&mut bus.cfg, addr)] = val;
}

// pub fn prg_read(bus: &mut Bus, addr: u16) -> u8 {
//   bus.prg[bus.cfg.prg.translate(addr as usize)]
// }
// pub fn prg_write(bus: &mut Bus, addr: u16, val: u8) {
//   bus.mapper.prg_write(&mut bus.cfg, addr as usize, val);
// }
// pub fn chr_read (bus: &mut Bus, addr: u16) -> u8 {
//   bus.chr[bus.cfg.chr.translate(addr as usize)]
// }
// pub fn chr_write (bus: &mut Bus, addr: u16, val: u8) {
//   bus.chr[bus.cfg.chr.translate(addr as usize)] = val;
// }
// pub fn vram_read(bus: &mut Bus, addr: u16) -> u8 {
//   bus.vram[bus.cfg.vram.translate(addr as usize)]
// }
// pub fn vram_write(bus: &mut Bus, addr: u16, val: u8) {
//   bus.vram[bus.cfg.vram.translate(addr as usize)] = val;
// }
pub fn vram0_read(bus: &mut Bus, addr: u16) -> u8 {
  bus.vram[addr as usize & 0x3FF]
}
pub fn vram0_write(bus: &mut Bus, addr: u16, val: u8) {
  bus.vram[addr as usize & 0x3FF] = val;
}
pub fn vram1_read(bus: &mut Bus, addr: u16) -> u8 {
  bus.vram[addr as usize & 0x3FF + 0x400]
}
pub fn vram1_write(bus: &mut Bus, addr: u16, val: u8) {
  bus.vram[addr as usize & 0x3FF + 0x400] = val;
}
pub fn chr_from_vram_read(bus: &mut Bus, addr: u16) -> u8 {
  bus.chr[bus.cfg.vram.translate(addr as usize)]
}
pub fn chr_from_vram_write(bus: &mut Bus, addr: u16, val: u8) {
  bus.chr[bus.cfg.vram.translate(addr as usize)] = val;
}

pub fn palette_read(bus: &mut Bus, addr: u16) -> u8 {
  if addr <= 0x3eff { return 0; }
  let ppu = bus.ctx.ppu();
  ppu.palettes[ppu.mirror_palette(addr) as usize]
}

pub fn palette_write(bus: &mut Bus, addr: u16, val: u8) {
  if addr <= 0x3eff { return; }
  let ppu = bus.ctx.ppu();
  // not masking val breaks the renderer for some reason
  ppu.palettes[ppu.mirror_palette(addr) as usize] = val & 0b0011_1111;
}

impl Default for MemMapping {  
  fn default() -> Self {
    // https://www.nesdev.org/wiki/CPU_memory_map

    let cpu_reads = [
      |bus: &mut Bus, addr| bus.ram[addr as usize & 0x7ff],
      |bus: &mut Bus, addr| bus.ctx.ppu().read_reg(addr & 0x2007),
      |bus: &mut Bus, addr| {
        match addr {
          0x4000..=0x4013 => bus.ctx.apu().read_reg(addr),
          0x4016 => bus.ctx.joypad().read1(),
          0x4017 => bus.ctx.joypad().read2(),
          0x4020..=0x5FFF => bus.mapper.cart_read(addr as usize),
          _ => 0,
        }
      },
      sram_read,
      prg_read, prg_read, prg_read, prg_read,
    ];

    let cpu_writes = [
      |bus: &mut Bus, addr, val| bus.ram[addr as usize & 0x7FF] = val,
      |bus: &mut Bus, addr, val| bus.ctx.ppu().write_reg(addr & 0x2007, val),
      |bus: &mut Bus, addr, val| {
        match addr {
          0x4000..=0x4013 => bus.ctx.apu().write_reg(addr as u16, val),
          0x4017 => {
            bus.ctx.apu().write_reg(addr as u16, val);
            bus.ctx.joypad().write(val);
          }
          0x4016 => bus.ctx.joypad().write(val),
          0x4014 => {
            bus.ctx.ppu().dma.init(val);
            bus.tick();

            while bus.ctx.ppu().dma.is_transfering() {
              let addr = bus.ctx.ppu().dma.current();
              let to_write = bus.cpu_read(addr);
              bus.tick();
              bus.cpu_write(0x2004, to_write);
              bus.tick();
            }
          }
          0x4015 => {
            bus.ctx.apu().write_reg(addr as u16, val);
            bus.tick();
            bus.handle_dmc();
          }
          0x4020..=0x5FFF => bus.mapper.cart_write(&mut bus.cfg, addr as usize, val),
          _ => {}
        }
      },
      sram_write,
      prg_write, prg_write, prg_write, prg_write,
    ];

    // https://www.nesdev.org/wiki/PPU_memory_map
    // 16 handlers, one for each 1kb

    let ppu_reads = [
      chr_read, chr_read, chr_read, chr_read, chr_read, chr_read, chr_read, chr_read,
      vram_read, vram_read, vram_read, vram_read,
      palette_read, palette_read, palette_read, palette_read,
    ];

    let ppu_writes = [
      chr_write, chr_write, chr_write, chr_write, chr_write, chr_write, chr_write, chr_write, 
      vram_write, vram_write, vram_write, vram_write, 
      palette_write, palette_write, palette_write, palette_write,
    ];

    Self { cpu_reads, cpu_writes, ppu_reads, ppu_writes }
  }
}

impl MemMapping {
  pub const SRAM_HANDLER: usize = 3;

  pub fn set_prg_handlers(&mut self, read: fn(&mut Bus, u16) -> u8, write: fn(&mut Bus, u16, u8)) {
    for i in 4..8 {
      self.cpu_reads[i]  = read;
      self.cpu_writes[i] = write;
    }
  }

  pub fn set_chr_handlers(&mut self, read: fn(&mut Bus, u16) -> u8, write: fn(&mut Bus, u16, u8)) {
    for i in 0..8 {
      self.ppu_reads[i]  = read;
      self.ppu_writes[i] = write;
    }
  }

  pub fn set_vram_handlers(&mut self, read: fn(&mut Bus, u16) -> u8, write: fn(&mut Bus, u16, u8)) {
    for i in 8..12 {
      self.ppu_reads[i]  = read;
      self.ppu_writes[i] = write;
    }
  }
}