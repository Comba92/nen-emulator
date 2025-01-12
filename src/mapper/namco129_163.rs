use crate::cart::{CartBanking, CartHeader, Mirroring};

use super::{Banking, Mapper};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Namco129_163 {
  irq_value: u16,
  irq_enabled: bool,
  irq_requested: Option<()>,

  chr_ram0_enabled: bool,
  chr_ram1_enabled: bool,
  sram_write_enabled: [bool; 4],

  apu_enabled: bool,
}

#[typetag::serde]
impl Mapper for Namco129_163 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.prg.set_page_to_last_bank(3);
    
    banks.chr = Banking::new_chr(header, 12);

    Box::new(Self{
      irq_enabled: false,
      irq_value: 0,
      irq_requested: None,
      chr_ram0_enabled: false,
      chr_ram1_enabled: false,
      sram_write_enabled: [false; 4],
      apu_enabled: false,
    })
  }

  fn cart_read(&mut self, addr: usize) -> u8 {
    match addr {
      0x5000..=0x57FFF => self.irq_value as u8,
      0x5800..=0x5FFFF => {
        let mut res = 0;
        res |= (self.irq_value >> 8) as u8;
        res |= (self.irq_enabled as u8) << 7;
        res
      }
      _ => 0xFF,
    }

  }

  fn cart_write(&mut self, _: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x5000..=0x57FFF => {
        self.irq_value = self.irq_value & 0xFF00 | val as u16;
        self.irq_requested = None;
      }
      0x5800..=0x5FFFF => {
        self.irq_value = 
          self.irq_value & 0x00FF | ((val as u16 & 0b0111_1111) << 8);
        self.irq_enabled = val >> 7 != 0;
        self.irq_requested = None;
      }
      _ => {}
    }
  }

  fn write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x8000..=0xDFFF => {
        if val <= 0xDF {
          let page = (addr as usize - 0x8000) / 0x800;
          banks.chr.set(page, val as usize);
        } else {
          let mirroring = match val % 2 == 0 {
            true  => Mirroring::SingleScreenA,
            false => Mirroring::SingleScreenB,
          };
          banks.vram.update(mirroring);
        }
      }
      0xE000..=0xE7FF => {
        let bank = val as usize & 0b11_1111;
        banks.prg.set(0, bank);
        self.apu_enabled = (val >> 6) & 1 == 0;
      }
      0xE800..=0xEFFF => {
        let bank = val as usize & 0b11_1111;
        banks.prg.set(1, bank);
        self.chr_ram0_enabled = (val >> 6) & 1 != 0;
        self.chr_ram1_enabled = (val >> 7) & 1 != 0;
      }
      0xF000..=0xF7FF => {
        let bank = val as usize & 0b11_1111;
        banks.prg.set(2, bank);
      }
      0xF800..=0xFFFF => {
        if val >> 6 == 0 {
          self.sram_write_enabled.fill(false);
        } else {
          for i in 0..self.sram_write_enabled.len() {
            self.sram_write_enabled[i] = val as usize >> i == 0; 
          }
        }
      }
      _ => {}
    }
  }

  fn notify_cpu_cycle(&mut self) {
    if self.irq_requested.is_some() { return; }

    self.irq_value += 1;
    if self.irq_value >= 0x7FFF {
      self.irq_requested = Some(());
    }
  }
}