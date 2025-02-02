use crate::cart::{CartBanking, CartHeader, Mirroring, PrgTarget};

use super::{set_byte_hi, set_byte_lo, Banking, Mapper};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct BandaiFCG {
  submapper: u8,
  eeprom: Box<[u8]>,

  irq_enabled: bool,
  irq_count: u16,
  irq_latch: u16,
  irq_requested: Option<()>,
}

#[typetag::serde]
impl Mapper for BandaiFCG {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);

    banks.chr = Banking::new_chr(header, 8);

    let eeprom = vec![0; 256].into_boxed_slice();
    Box::new(Self{
      submapper: header.submapper,
      eeprom,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match (addr, self.submapper) {
      (0x6000..=0x7FFF, 5) => {
        // submapper 5 eeprom read
      }
      
      (0x6000..=0x6007 | 0x8000..=0x8007, _) => {
        let page = addr & 0x07;
        banks.chr.set_page(page, val as usize);
      }

      (0x6008 | 0x8008, _) => 
        banks.prg.set_page(0, val as usize & 0b1111),

      (0x6009 | 0x8009, _) => {
        let mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };
        banks.ciram.update(mirroring);
      }

      (0x600A | 0x800A, _) =>  {
        self.irq_enabled = val & 1 != 0;
        self.irq_requested = None;

        if self.submapper == 5 || addr == 0x800A {
          self.irq_count = self.irq_latch;
        }
      }
      (0x600B, _) => self.irq_count = set_byte_hi(self.irq_count, val),
      (0x600C, _) => self.irq_count = set_byte_lo(self.irq_count, val),

      (0x800B, _) => self.irq_latch = set_byte_hi(self.irq_latch, val),
      (0x800C, _) => self.irq_latch = set_byte_lo(self.irq_latch, val),

      (0x800D, _) => {
        // submapper 5 eeprom ctrl
      }
        _ => {}
    }
  }

  fn map_prg_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::Prg(addr),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!(),
    }
  }

  fn notify_cpu_cycle(&mut self) {
    if !self.irq_enabled { return; }

    if self.irq_count == 0 {
      self.irq_requested = Some(());
    }
    self.irq_count = self.irq_count.wrapping_sub(1);
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}