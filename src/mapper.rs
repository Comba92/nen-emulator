use crate::{bus::{Banking, BankingHandler, ChrBank, MemHandler}, cart::CartHeader, emu::Mirroring};

// https://www.nesdev.org/wiki/Mapper
pub trait Mapper {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> where Self: Sized;
  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8);
  fn step(&mut self) {}

  // temporary solution
  fn notify_chr_access(&mut self, _: u16, banks: &mut BankingHandler) {}
}

pub fn mapper_from_header(header: &CartHeader, banks: &mut BankingHandler) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header, banks),
    1 => MMC1::new(header, banks),
    2 => UxROM::new(header, banks),
    3 => CNROM::new(header, banks),
    7 => AxROM::new(header, banks),
    9 => MMC2::new(header, banks),
    66 => GxROM::new(header, banks),
    71 => Codemasters::new(header, banks),
    _ => return Err(format!("mapper {} not implemented", header.mapper)),
  };

  Ok(mapper)
}

// https://www.nesdev.org/wiki/NROM
struct NROM;
impl Mapper for NROM {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {    
    if header.prg_size <= 16 * 1024 {
      banks.prg.set_page(1, 0);
    } else {
      banks.prg.set_page(1, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut BankingHandler, _: u16, _: u8) {}
}

// https://www.nesdev.org/wiki/UxROM
struct UxROM; 
impl Mapper for UxROM {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);
    banks.chr = Banking::new_chr(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, _: u16, val: u8) {
    banks.prg.set_page(0, val & 0b111);
  }
}

// https://www.nesdev.org/wiki/CNROM
struct CNROM;
impl Mapper for CNROM {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 1);
    banks.chr = Banking::new_chr(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, _: u16, val: u8) {
    banks.chr.set_page(0, val & 0b11);
  }
}

// https://www.nesdev.org/wiki/GxROM
struct GxROM;
impl Mapper for GxROM {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, _: u16, val: u8) {
    banks.prg.set_page(0, (val >> 4) & 0b11);
    banks.chr.set_page(0, val & 0b11);
  }
}

// https://www.nesdev.org/wiki/AxROM
struct AxROM;
impl Mapper for AxROM {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> where Self: Sized {
    banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8) {
    banks.prg.set_page(0, val & 0b111);
    
    let mirroring = if val & 0x10 == 0 {
      Mirroring::SingleScreenA
    } else {
      Mirroring::SingleScreenB
    };
    banks.vram.mirror(&mirroring);
  }
}

#[derive(Default)]
struct MMC1 {
  shift_reg: u8,
  shift_count: u8,
  prg_bank: u8,
  chr_bank: u8,
  prg_mode: u8,
  chr_8kb_mode: bool,
  prg_bank_mask: u8,
  chr_bank_mask: u8,
}
impl Mapper for MMC1 {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> where Self: Sized {
    banks.chr = Banking::new_chr(header, 2);
    banks.prg.set_page(0, 0);
    banks.prg.set_page_to_last_bank(1);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8) {
    if val & 0x80 != 0 {
      self.shift_reg = 0;
      self.shift_count = 0;
      banks.prg.change(2);
      banks.prg.set_page(0, self.prg_bank);
      self.prg_bank_mask = 0;
      
      return;
    }

    self.shift_reg |= (val & 1) << self.shift_count;
    self.shift_count += 1;

    if self.shift_count < 5 { return; }

    let val = self.shift_reg;
    self.shift_reg = 0;
    self.shift_count = 0;

    match addr {
      // 0x8000..0x9ffff
      0x8000..=0x9fff => {
        let mirroring = match val & 0b11 {
          0 => Mirroring::SingleScreenA,
          1 => Mirroring::SingleScreenB,
          2 => Mirroring::Vertical,
          _ => Mirroring::Horizontal
        };
        banks.vram.mirror(&mirroring);
        
        self.prg_mode = (val >> 2) & 0b11;
        self.prg_bank_mask = match self.prg_mode {
          2 => {
            banks.prg.change(2);
            banks.prg.set_page(0, 0);
            banks.prg.set_page(1, self.prg_bank);
            0
          }
          3 => {
            banks.prg.change(2);
            banks.prg.set_page(0, self.prg_bank);
            banks.prg.set_page_to_last_bank(1);
            0
          }
          _ => {
            banks.prg.change(1);
            banks.prg.set_page(0, self.prg_bank & !1);
            1
          }
        };

        self.chr_8kb_mode = val & 0x80 == 0;
        self.chr_bank_mask = if self.chr_8kb_mode {
          banks.chr.change(1);
          1
        } else {
          banks.chr.change(2);
          0
        };
      }
      // 0xa000..0xbfff
      0xa000..=0xbfff => {
        self.chr_bank = val & !self.chr_bank_mask; 
        banks.chr.set_page(0, self.chr_bank);
      }
      // 0xc000..0xdfff
      0xc000..=0xdfff => if !self.chr_8kb_mode {
        self.chr_bank = val;
        banks.chr.set_page(1, self.chr_bank);
      }
      // 0xe000..0xffff
      0xe000..=0xffff => { 
        self.prg_bank = val & !self.prg_bank_mask;
        match self.prg_mode {
          2 => banks.prg.set_page(1, self.prg_bank),
          3 => banks.prg.set_page(0, self.prg_bank),
          _ => banks.prg.set_page(0 as usize, self.prg_bank),
        }
      }
      _ => {}
    } 
  }
}

enum MMC2Latch {
  FD, FE
}
struct MMC2 {
  bank_fd: Banking<ChrBank>,
  bank_fe: Banking<ChrBank>,
  latch0: MMC2Latch,
  latch1: MMC2Latch,
}

impl Mapper for MMC2 {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> where Self: Sized {
    banks.prg = Banking::new_prg(header, 4);
    let last_bank = (banks.prg.banks_count - 1) as u8;
    banks.prg.set_page(1, last_bank-2);
    banks.prg.set_page(2, last_bank-1);
    banks.prg.set_page(3, last_bank);

    banks.chr = Banking::new_chr(header, 2);

    Box::new(Self {
      bank_fd: Banking::new_chr(header, 2),
      bank_fe: Banking::new_chr(header, 2),
      latch0: MMC2Latch::FD,
      latch1: MMC2Latch::FD,
    })
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8) {
    match addr {
      0xa000..=0xafff => banks.prg.set_page(0, val & 0xf),
      0xb000..=0xbfff => self.bank_fd.set_page(0, val & 0x1f),
      0xc000..=0xcfff => self.bank_fe.set_page(0, val & 0x1f),
      0xd000..=0xdfff => self.bank_fd.set_page(1, val & 0x1f),
      0xe000..=0xefff => self.bank_fe.set_page(1, val & 0x1f),
      0xf000..=0xffff => {
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal
        };

        banks.vram.mirror(&mirroring);
      }
      _ => {}
    }
  }


  // TODO: temporary solution
  fn notify_chr_access(&mut self, addr: u16, banks: &mut BankingHandler) {
    match addr {
      0x0fd8 => self.latch0 = MMC2Latch::FD,
      0x0fe8 => self.latch0 = MMC2Latch::FE,
      0x1fd8..=0x1fdf => self.latch1 = MMC2Latch::FD, 
      0x1fe8..=0x1fef => self.latch1 = MMC2Latch::FE,
      _ => {}
    }

    match self.latch0 {
      MMC2Latch::FD => banks.chr.bankings[0] = self.bank_fd.bankings[0],
      MMC2Latch::FE => banks.chr.bankings[0] = self.bank_fe.bankings[0],
    }

    match self.latch1 {
      MMC2Latch::FD => banks.chr.bankings[1] = self.bank_fd.bankings[1],
      MMC2Latch::FE => banks.chr.bankings[1] = self.bank_fe.bankings[1],
    }
  }
}

struct Codemasters;
impl Mapper for Codemasters {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);

    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8) {
    match addr {
      0xc000..=0xffff => banks.prg.set_page(0, val & 0b1111),
      _ => {}
    }
  }
}