use crate::cart::{CartBanking, CartHeader, Mirroring, PrgTarget};

use super::{mmc3::MMC3, Banking, Mapper};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper091 {
  submapper: u8,
  mmc3: MMC3,
}

#[typetag::serde]
impl Mapper for INesMapper091 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    let mmc3 = *MMC3::new(header, banks);

    banks.prg = Banking::new_prg(header, 4);
    banks.prg.set_page(2, banks.prg.banks_count-2);
    banks.prg.set_page(3, banks.prg.banks_count-1);

    banks.chr = Banking::new_chr(header, 4);

    Box::new(Self {
      submapper: header.submapper,
      mmc3,
    })
  }

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    let mask = if self.submapper == 1 { 0xF007 } else { 0xF003 }; 
    
    match addr & mask {
      0x6000..=0x6003 => {
        let page = addr - 0x6000;
        banks.chr.set_page(page, val as usize);
      }
      0x6004 => banks.ciram.update(Mirroring::Horizontal),
      0x6005 => banks.ciram.update(Mirroring::Vertical),

      0x6006 => self.mmc3.irq_latch = (self.mmc3.irq_latch & 0xF0) | (val & 0xF0),
      0x6007 => self.mmc3.irq_latch = (self.mmc3.irq_latch & 0x0F) | ((val & 0x0F) << 4),

      0x7000 => banks.prg.set_page(0, val as usize),
      0x7001 => banks.prg.set_page(1, val as usize),

      0x7006 => {
        self.mmc3.irq_enabled = false;
        self.mmc3.irq_requested = None;
      }
      0x7007 => {
        self.mmc3.irq_enabled = true;
        self.mmc3.irq_reload = true;
      }

      0x8000..=0x9FFF => {

      }
      _ => {}
    }
  }

  fn map_prg_addr(&self, banks: &mut CartBanking, addr:usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::Prg(addr),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!()
    }
  }

  fn notify_scanline(&mut self) {
    self.mmc3.notify_scanline();
  }

  fn poll_irq(&mut self) -> bool {
    self.mmc3.poll_irq()
  }
}