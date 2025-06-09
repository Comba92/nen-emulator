use crate::{
  banks::MemConfig,
  cart::{CartHeader, Mirroring},
};

use super::{konami_irq::KonamiIrq, Banking, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct VRC7 {
  irq: KonamiIrq,
  sram_enabled: bool,
}

impl VRC7 {
  fn update_chr_banks(&self, banks: &mut MemConfig, addr: usize, val: u8) {
    let val = val as usize;
    match addr {
      0xA000 => banks.chr.set_page(0, val),
      0xA008 | 0xA010 => banks.chr.set_page(1, val),
      0xB000 => banks.chr.set_page(2, val),
      0xB008 | 0xB010 => banks.chr.set_page(3, val),
      0xC000 => banks.chr.set_page(4, val),
      0xC008 | 0xC010 => banks.chr.set_page(5, val),
      0xD000 => banks.chr.set_page(6, val),
      0xD008 | 0xD010 => banks.chr.set_page(7, val),
      _ => {}
    }
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for VRC7 {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.prg.set_page_to_last_bank(3);
    banks.chr = Banking::new_chr(header, 8);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x8000 => banks.prg.set_page(0, val as usize & 0b0011_1111),
      0x8010 | 0x8008 => banks.prg.set_page(1, val as usize & 0b0011_1111),
      0x9000 => banks.prg.set_page(2, val as usize & 0b0011_1111),
      0xA000..=0xDFFF => self.update_chr_banks(banks, addr, val),
      0xE000 => {
        let mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };

        banks.vram.update(mirroring);
        self.sram_enabled = val >> 7 != 0;
      }
      0xE008 | 0xE010 => self.irq.latch = val as u16,
      0xF000 => self.irq.write_ctrl(val),
      0xF008 | 0xF010 => self.irq.write_ack(),
      _ => {}
    }
  }

  fn notify_cpu_cycle(&mut self) {
    self.irq.handle_irq();
  }

  fn poll_irq(&mut self) -> bool {
    self.irq.requested.is_some()
  }
}
