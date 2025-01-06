use crate::cart::{CartHeader, Mirroring};

use super::{Banking, ChrBanking, Mapper, PrgBanking, RamBanking};

#[derive(Default, PartialEq, serde::Serialize, serde::Deserialize)]
enum PrgMode { Bank32kb, FixFirstPage, #[default] FixLastPage }
#[derive(Default, PartialEq, serde::Serialize, serde::Deserialize)]
enum ChrMode { #[default] Bank8kb, Bank4kb }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct MMC1 {
  prg_select: usize,
  has_512kb_prg: bool,
  chr_select0: usize,
  chr_select1: usize,
  last_wrote_chr_select1: bool,

  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
  sram_banks: Banking<RamBanking>,

  shift_reg: u8,
  shift_writes: usize,
  mirroring: Mirroring,
  prg_mode: PrgMode,
  chr_mode: ChrMode,
}

impl MMC1 {
  fn write_ctrl(&mut self, val: u8) {
    self.mirroring = match val & 0b11 {
      0 => Mirroring::SingleScreenA,
      1 => Mirroring::SingleScreenB,
      2 => Mirroring::Vertical,
      _ => Mirroring::Horizontal,
    };

    self.prg_mode = match (val >> 2) & 0b11 {
      2 => PrgMode::FixFirstPage,
      3 => PrgMode::FixLastPage,
      _ => PrgMode::Bank32kb,
    };
    self.update_prg_banks();

    self.chr_mode = match (val >> 4) != 0 {
      false => ChrMode::Bank8kb,
      true  => ChrMode::Bank4kb,
    };
    self.update_chr_and_sram_banks();
  }

  fn update_prg_banks(&mut self) {
    let (bank0, mut bank1) = match self.prg_mode {
      PrgMode::Bank32kb => {
        let bank = self.prg_select >> 1;
        (bank, bank+1)
      },
      PrgMode::FixFirstPage => (0, self.prg_select),
      PrgMode::FixLastPage => 
        (self.prg_select, self.prg_banks.banks_count-1),
    };
    
    let bank_256kb = if self.has_512kb_prg {
      if bank1 == self.prg_banks.banks_count-1 {
        bank1 = self.prg_banks.banks_count/2 - 1;
      }
      self.sxrom_select() & 0b1_0000
    } else { 0 };
    
    self.prg_banks.set(0, bank0 + bank_256kb);
    self.prg_banks.set(1, bank1 + bank_256kb);
  }

  fn sxrom_select(&self) -> usize {
      // SxRom register at 0xA000 and 0xC000
      if self.last_wrote_chr_select1 
      && self.chr_mode == ChrMode::Bank4kb 
      {
        self.chr_select1
      } else { self.chr_select0 }
  }

  fn update_chr_and_sram_banks(&mut self) {
    match self.chr_mode {
      ChrMode::Bank8kb => {
        let bank = self.chr_select0 >> 1;
        self.chr_banks.set(0, bank);
        self.chr_banks.set(1, bank+1);
      }
      ChrMode::Bank4kb => {
        self.chr_banks.set(0, self.chr_select0);
        self.chr_banks.set(1, self.chr_select1);
      }
    }

    let sram_select = self.sxrom_select();
    const KB8: usize  = 8 * 1024;
    const KB16: usize = 16 * 1024;
    const KB32: usize = 32 * 1024;
    let bank = match self.sram_banks.data_size {
        KB8 => 0,
        KB16 => (sram_select >> 3) & 0b01,
        KB32 => (sram_select >> 2) & 0b11,
        _ => 0,
    };
    self.sram_banks.set(0, bank);
  }
}

#[typetag::serde]
impl Mapper for MMC1 {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 2);
    let mut chr_banks = Banking::new_chr(header, 2);
    let sram_banks = Banking::new_sram(header);
    
    // mode 3 by default
    if header.prg_size > 256 * 1024 {
      prg_banks.set(1, prg_banks.banks_count/2 -1);
    } else {
      prg_banks.set(1, prg_banks.banks_count-1);
    }

    // bank 8kb by default
    chr_banks.set(0, 0);
    chr_banks.set(1, 1);

    let mapper = Self {
      prg_select: 0,
      has_512kb_prg: header.prg_size > 256 * 1024,
      chr_select0: 0,
      chr_select1: 0,
      prg_banks,
      chr_banks,
      sram_banks,

      last_wrote_chr_select1: false,
      shift_reg: 0,
      shift_writes: 0,
      mirroring: Default::default(),
      prg_mode: Default::default(),
      chr_mode: Default::default(),
    };

    Box::new(mapper)
  }
  
  fn write(&mut self,addr:usize,val:u8) {
    if val & 0b1000_0000 != 0 {
      self.shift_reg = 0;
      self.shift_writes = 0;
      self.prg_mode = PrgMode::FixLastPage;
    } else if self.shift_writes < 5 {
      self.shift_reg = (self.shift_reg >> 1) | ((val & 1) << 4);
      self.shift_writes += 1;
    }
    
    if self.shift_writes >= 5 {
      match addr {
        0x8000..=0x9FFF => self.write_ctrl(self.shift_reg),
        0xA000..=0xBFFF => {
          self.chr_select0 = self.shift_reg as usize & 0b1_1111;
          self.update_chr_and_sram_banks();
          self.last_wrote_chr_select1 = false;
        }
        0xC000..=0xDFFF => {
          self.chr_select1 = self.shift_reg as usize & 0b1_1111;
          self.update_chr_and_sram_banks();
          self.last_wrote_chr_select1 = true;
        }
        0xE000..=0xFFFF => {
          self.prg_select  = self.shift_reg as usize & 0b1111;
          self.update_prg_banks();
        }
        _ => unreachable!("Accessed {addr:x}")
      }
      
      self.shift_writes = 0;
      self.shift_reg = 0;
    }
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn sram_read(&self, ram: &[u8], addr: usize) -> u8 {
    ram[self.sram_banks.addr(addr)]
  }

  fn sram_write(&mut self, ram: &mut[u8], addr: usize, val: u8) {
    ram[self.sram_banks.addr(addr)] = val;
  }

  fn mirroring(&self) -> Option<Mirroring> {
    Some(self.mirroring)
  }
}