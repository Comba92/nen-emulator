use crate::{
  banks::MemConfig,
  cart::{CartHeader, Mirroring},
};

use super::{Banking, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, PartialEq)]
enum PrgMode {
  Bank32kb,
  FixFirstPage,
  #[default]
  FixLastPage,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, PartialEq)]
enum ChrMode {
  #[default]
  Bank8kb,
  Bank4kb,
}

// Mapper 01
// https://www.nesdev.org/wiki/MMC1
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct MMC1 {
  prg_select: usize,
  has_512kb_prg: bool,
  prg_256kb_bank: usize,
  prg_last_bank: usize,
  chr_select0: usize,
  chr_select1: usize,
  last_wrote_chr_select1: bool,

  shift_reg: u8,
  shift_writes: usize,
  write_lock_delay: u8,
  prg_mode: PrgMode,
  chr_mode: ChrMode,
}

impl MMC1 {
  fn write_ctrl(&mut self, cfg: &mut MemConfig, val: u8) {
    let mirroring = match val & 0b11 {
      0 => Mirroring::SingleScreenA,
      1 => Mirroring::SingleScreenB,
      2 => Mirroring::Vertical,
      _ => Mirroring::Horizontal,
    };
    cfg.vram.update(mirroring);

    self.prg_mode = match (val >> 2) & 0b11 {
      2 => PrgMode::FixFirstPage,
      3 => PrgMode::FixLastPage,
      _ => PrgMode::Bank32kb,
    };
    self.update_prg_banks(cfg);

    self.chr_mode = match (val >> 4) != 0 {
      false => ChrMode::Bank8kb,
      true => ChrMode::Bank4kb,
    };
    self.update_all_banks(cfg);
  }

  fn update_prg_banks(&self, cfg: &mut MemConfig) {
    let (bank0, bank1) = match self.prg_mode {
      PrgMode::Bank32kb => {
        let bank = self.prg_select & !1;
        (bank, bank + 1)
      }
      PrgMode::FixFirstPage => (0, self.prg_select),
      PrgMode::FixLastPage => (self.prg_select, self.prg_last_bank),
    };

    cfg.prg.set_page(0, bank0 | self.prg_256kb_bank);
    cfg.prg.set_page(1, bank1 | self.prg_256kb_bank);
  }

  fn update_all_banks(&mut self, cfg: &mut MemConfig) {
    match self.chr_mode {
      ChrMode::Bank8kb => {
        let bank = self.chr_select0 & !1;
        cfg.chr.set_page(0, bank);
        cfg.chr.set_page(1, bank + 1);
      }
      ChrMode::Bank4kb => {
        cfg.chr.set_page(0, self.chr_select0);
        cfg.chr.set_page(1, self.chr_select1);
      }
    }

    // SxRom register at 0xA000 and 0xC000
    let sxrom_select = if self.last_wrote_chr_select1 && self.chr_mode == ChrMode::Bank4kb {
      self.chr_select1
    } else {
      self.chr_select0
    };

    if self.has_512kb_prg {
      self.prg_256kb_bank = sxrom_select & 0b1_0000;
      self.update_prg_banks(cfg);
    }

    const KB8: usize = 8 * 1024;
    const KB16: usize = 16 * 1024;
    const KB32: usize = 32 * 1024;
    let bank = match cfg.sram.data_size {
      KB8 => 0,
      KB16 => (sxrom_select >> 3) & 0b01,
      KB32 => (sxrom_select >> 2) & 0b11,
      _ => 0,
    };
    cfg.sram.set_page(0, bank);
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for MMC1 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 2);
    cfg.chr = Banking::new_chr(header, 2);
    cfg.sram = Banking::new_sram(header);

    let has_512kb_prg = header.prg_size > 256 * 1024;

    // 512kb prg roms acts as if they only have 256kb, so the last prg bank counts should be half
    let prg_last_bank = if has_512kb_prg {
      cfg.prg.banks_count / 2 - 1
    } else {
      cfg.prg.banks_count - 1
    };

    // mode 3 by default
    cfg.prg.set_page(1, cfg.prg.banks_count - 1);

    // bank 8kb by default
    cfg.chr.set_page(0, 0);
    cfg.chr.set_page(1, 1);

    let mapper = Self {
      prg_last_bank,
      has_512kb_prg,
      ..Default::default()
    };

    Box::new(mapper)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, addr: usize, val: u8) {
    if self.write_lock_delay > 0 {
      self.write_lock_delay = 2;
      return;
    }

    if val & 0b1000_0000 != 0 {
      self.shift_reg = 0;
      self.shift_writes = 0;
      self.prg_mode = PrgMode::FixLastPage;
      self.update_prg_banks(cfg);
    } else if self.shift_writes < 5 {
      self.shift_reg = (self.shift_reg >> 1) | ((val & 1) << 4);
      self.shift_writes += 1;
    }

    self.write_lock_delay = 2;

    if self.shift_writes >= 5 {
      match addr {
        0x8000..=0x9FFF => self.write_ctrl(cfg, self.shift_reg),
        0xA000..=0xBFFF => {
          self.chr_select0 = self.shift_reg as usize & 0b1_1111;
          self.last_wrote_chr_select1 = false;
          self.update_all_banks(cfg);
        }
        0xC000..=0xDFFF => {
          self.chr_select1 = self.shift_reg as usize & 0b1_1111;
          self.last_wrote_chr_select1 = true;
          self.update_all_banks(cfg);
        }
        0xE000..=0xFFFF => {
          self.prg_select = self.shift_reg as usize & 0b1111;
          self.update_prg_banks(cfg);
        }
        _ => {}
      }

      self.shift_writes = 0;
      self.shift_reg = 0;
    }
  }

  fn notify_cpu_cycle(&mut self) {
    if self.write_lock_delay > 0 {
      self.write_lock_delay -= 1;
    }
  }
}
