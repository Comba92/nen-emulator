use crate::{banks::MemConfig, mem::{self, MemMapping}, cart::CartHeader};

use super::{Banking, Mapper};

// Mapper 111
// https://www.nesdev.org/wiki/GTROM
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GTROM;
impl GTROM {
  fn write(&mut self, cfg: &mut MemConfig, val: u8) {
    cfg.prg.set_page(0, val as usize & 0b1111);
    cfg.chr.set_page(0, (val >> 4) as usize & 1);
    // The nametables can select between the last two 8KiB of the PPU RAM
    cfg.vram.set_page(0, ((val >> 5) as usize & 1) + 2);

    cfg.mapping.cpu_writes[MemMapping::SRAM_HANDLER] = mem::prg_write;

    for i in 12..16 {
      cfg.mapping.ppu_reads[i]  = mem::vram_read;
      cfg.mapping.ppu_writes[i] = mem::vram_write;
    }
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for GTROM {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 1);
    cfg.chr = Banking::new_chr(header, 1);
    cfg.vram = Banking::new(header.chr_real_size(), 0x2000, 8*1024, 1);

    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, addr: usize, val: u8) {
    if (0x7000..=0x7FFF).contains(&addr) {
      self.write(cfg, val);
    }
  }
  fn cart_write(&mut self, cfg: &mut MemConfig, addr: usize, val:u8) {
    if (0x5000..=0x5FFF).contains(&addr) {
      self.write(cfg, val);
    }
  }

  // fn map_prg_addr_branching(&mut self, cfg: &mut MemConfig, addr: usize) -> PrgTarget {
  //   match addr {
  //     0x6000..=0x7FFF => PrgTarget::Prg(addr),
  //     0x8000..=0xFFFF => PrgTarget::Prg(cfg.prg.translate(addr)),
  //     _ => unreachable!()
  //   }
  // }

  // fn map_ppu_addr_branching(&mut self, cfg: &mut MemConfig, addr: usize) -> PpuTarget {
  //   match addr {
  //     0x0000..=0x1FFF => PpuTarget::Chr(cfg.chr.translate(addr)),
  //     // this thing uses the vram mirrors as additional ram
  //     0x2000..=0x3FFF => PpuTarget::Chr(cfg.ciram.translate(addr)),
  //     _ => unreachable!()
  //   }
  // }
}
