use crate::cart::{MemConfig, CartHeader};
use super::{konami_irq::{self, KonamiIrq}, Banking, Mapper};

// Mapper 73
// https://www.nesdev.org/wiki/VRC3
#[derive(serde::Serialize, serde::Deserialize)]
pub struct VRC3 {
  irq: KonamiIrq,
}

#[typetag::serde]
impl Mapper for VRC3 {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);
    Box::new(Self{irq: Default::default()})
  }

  fn prg_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0xF000..=0xFFFF => banks.prg.set_page(0, val as usize & 0b111),
      0x8000..=0x8FFF => self.irq.latch = (self.irq.latch & 0xFFF0) | ((val as u16 & 0b1111)),
      0x9000..=0x9FFF => self.irq.latch = (self.irq.latch & 0xFF0F) | ((val as u16 & 0b1111) << 4),
      0xA000..=0xAFFF => self.irq.latch = (self.irq.latch & 0xF0FF) | ((val as u16 & 0b1111) << 8),
      0xB000..=0xBFFF => self.irq.latch = (self.irq.latch & 0x0FFF) | ((val as u16 & 0b1111) << 12),
      0xC000..=0xCFFF => self.irq.write_ctrl(val),
      0xD000..=0xDFFF => self.irq.write_ack(),
      _ => {}
    }
  }

  fn notify_cpu_cycle(&mut self) {
    if !self.irq.enabled { return; }
    
    let (_, overflow8) = (self.irq.count as u8).overflowing_add(1);
    let (_, overflow16) = self.irq.count.overflowing_add(1);

    match self.irq.mode {
      konami_irq::IrqMode::Mode0 => {
        if overflow16 {
          self.irq.count = self.irq.latch;
          self.irq.requested = Some(())
        } else {
          self.irq.count += 1;
        }
      }
      konami_irq::IrqMode::Mode1 => {
        if overflow8 {
          self.irq.count = 
            (self.irq.count & 0xFF00) | (self.irq.latch & 0x00FF);
          self.irq.requested = Some(())
        } else {
          self.irq.count += 1;
        }
      }
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.irq.requested.is_some()
  }
}