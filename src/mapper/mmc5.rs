use crate::{cart::{CartBanking, CartHeader, Mirroring, PpuTarget, PrgTarget}, ppu::PpuState};
use super::{Banking, ChrBanking, Mapper};

#[derive(Default, PartialEq, serde::Serialize, serde::Deserialize)]
enum PrgMode { Bank32kb, Bank16kb, BankMixed, #[default] Bank8kb }

#[derive(Default, serde::Serialize, serde::Deserialize)]
enum ChrMode { Bank8kb, Bank4kb, Bank2kb, #[default] Bank1kb }

#[derive(Default, serde::Serialize, serde::Deserialize)]
enum ExRamMode { Nametbl, NametblEx, ReadWrite, #[default] ReadOnly }

#[derive(Copy, Clone, Default, serde::Serialize, serde::Deserialize)]
enum NametblMapping { #[default] CiRam0, CiRam1, ExRam, FillMode }

#[derive(Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
enum AccessTarget { #[default] Prg, SRam }



// Mapper 5
// https://www.nesdev.org/wiki/MMC5
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct MMC5 {
  ppu_spr_16: bool,
  ppu_data_sub: bool,
  ppu_state: PpuState,
  
  prg_mode: PrgMode,
  prg_selects: [(AccessTarget, usize); 5],

  sram_write_lock1: bool,
  sram_write_lock2: bool,

  chr_mode: ChrMode,
  chr_selects: [u8; 12],
  bg_banks: Banking<ChrBanking>, 
  last_selected_bg_regs: bool,
  chr_bank_hi: u8,
  
  exram_mode: ExRamMode,
  exram: Box<[u8]>,

  ciram_mapping: [NametblMapping; 4],
  fill_mode_tile: u8,
  fill_mode_color: u8,

  irq_enabled: bool,
  irq_pending: bool,
  irq_value: u8,
  irq_count: u8,
  irq_requested: Option<()>,
  ppu_in_frame: bool,

  mirroring: Mirroring,

  multiplicand: u8,
  multiplier: u8,
}

impl MMC5 {
  fn notify_nmi(&mut self) {
    self.ppu_in_frame = false;
    self.irq_pending = false;
    self.irq_requested = None;
    self.irq_count = 0;
  }

  fn in_8x16_mode(&self) -> bool {
    self.ppu_spr_16 && self.ppu_data_sub
  }

  fn set_prg_page(&self, banks: &mut CartBanking, reg: usize, page: usize) {
    let (target, bank) = self.prg_selects[reg];
    match target {
      AccessTarget::Prg => banks.prg.set_page(page, bank),
      AccessTarget::SRam => banks.sram.set_page(page+1, bank),
    }
  }

  fn set_prg_page2(&self, banks: &mut CartBanking, reg: usize, page: usize) {
    let (target, bank) = self.prg_selects[reg];
    let bank = bank & !1;
    match target {
      AccessTarget::Prg => {
        banks.prg.set_page(page, bank);
        banks.prg.set_page(page+1, bank | 1);
      }
      AccessTarget::SRam => {
        banks.sram.set_page(page+1, bank);
        banks.sram.set_page(page+2, bank | 1);
      },
    }
  }

  fn update_prg_and_sram_banks(&mut self, banks: &mut CartBanking) {
    // this is always the same
    banks.sram.set_page(0, self.prg_selects[0].1);

    // Register 5114, only used in mode3
    if self.prg_mode == PrgMode::Bank8kb {
      self.set_prg_page(banks, 1, 0);
    }

    // Register 5115, used in all modes except mode0
    if self.prg_mode == PrgMode::Bank8kb {
      self.set_prg_page(banks, 2, 1);
    } else if self.prg_mode == PrgMode::Bank8kb || self.prg_mode == PrgMode::BankMixed {
      self.set_prg_page2(banks, 2, 0);
    }

    // Register 5116 only used in modes 2 and 3
    if self.prg_mode == PrgMode::Bank8kb || self.prg_mode == PrgMode::BankMixed {
      self.set_prg_page(banks, 3, 2);
    }

    // Register 5117 used in all modes
    if self.prg_mode == PrgMode::Bank8kb || self.prg_mode == PrgMode::BankMixed {
      self.set_prg_page(banks, 4, 3);
    } else if self.prg_mode == PrgMode::Bank16kb {
      self.set_prg_page2(banks, 4, 2);
    } else {
      let bank = self.prg_selects[4].1 & !0b11;
      banks.prg.set_page(0, bank);
      banks.prg.set_page(1, bank | 1);
      banks.prg.set_page(2, bank | 2);
      banks.prg.set_page(3, bank | 3); 
    }
  }

  fn update_chr_banks(&mut self, banks: &mut CartBanking) {
    match self.chr_mode {
      ChrMode::Bank8kb => {
        let bank = self.chr_selects[7] as usize;
        for page in 0..8 {
          banks.chr.set_page(page, bank + page);
        }
      }
      ChrMode::Bank4kb => {
        let bank = self.chr_selects[4] as usize;
        for page in 0..4 {
          banks.chr.set_page(page, bank + page);
        }

        let bank = self.chr_selects[7] as usize;
        for page in 4..8 {
          banks.chr.set_page(page, bank + (page-4));
        }
      }
      ChrMode::Bank2kb => {
        let bank = self.chr_selects[1] as usize;
        banks.chr.set_page(0, bank);
        banks.chr.set_page(1, bank + 1);


        let bank = self.chr_selects[3] as usize;
        banks.chr.set_page(2, bank);
        banks.chr.set_page(3, bank + 1);

        let bank = self.chr_selects[5] as usize;
        banks.chr.set_page(4, bank);
        banks.chr.set_page(5, bank + 1);

        let bank = self.chr_selects[7] as usize;
        banks.chr.set_page(6, bank);
        banks.chr.set_page(7, bank + 1);
      }
      ChrMode::Bank1kb => {
        for i in 0..8 {
          banks.chr.set_page(i, self.chr_selects[i] as usize);
        }
      }
    }

    match self.chr_mode {
      ChrMode::Bank8kb => {
        let bank = self.chr_selects[11] as usize;
        for page in 0..8 {
          self.bg_banks.set_page(page, bank + page);
        }
      }
      ChrMode::Bank4kb => {
        let bank = self.chr_selects[11] as usize;
        for page in 0..4 {
          self.bg_banks.set_page(page, bank + page);
        }

        let bank = self.chr_selects[11] as usize;
        for page in 4..8 {
          self.bg_banks.set_page(page, bank + (page-4));
        }
      }
      ChrMode::Bank2kb => {
        let bank = self.chr_selects[9] as usize;
        self.bg_banks.set_page(0, bank);
        self.bg_banks.set_page(1, bank + 1);


        let bank = self.chr_selects[11] as usize;
        self.bg_banks.set_page(2, bank);
        self.bg_banks.set_page(3, bank + 1);

        let bank = self.chr_selects[9] as usize;
        self.bg_banks.set_page(4, bank);
        self.bg_banks.set_page(5, bank + 1);

        let bank = self.chr_selects[11] as usize;
        self.bg_banks.set_page(6, bank);
        self.bg_banks.set_page(7, bank + 1);
      }
      ChrMode::Bank1kb => {
        for i in 0..8 {
          self.bg_banks.set_page(i, self.chr_selects[8 + (i % 4)] as usize);
        }
      }
    }
    
  }

  fn update_ciram_banks(&mut self, banks: &mut CartBanking) {
    for (page, nametbl) in self.ciram_mapping.iter().enumerate() {
      match nametbl {
        NametblMapping::CiRam0 => banks.ciram.set_page(page, 0),
        NametblMapping::CiRam1 => banks.ciram.set_page(page, 1),
        _ => {}
      }
    }
  }
}

#[typetag::serde]
impl Mapper for MMC5 {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self>  {
    banks.prg = Banking::new_prg(header, 4);
    banks.chr = Banking::new_chr(header, 8);
    let bg_banks = Banking::new_chr(header, 8);
    banks.sram = Banking::new(header.sram_real_size(), 0x6000, 8*1024, 4);


    let mut prg_selects = [const { (AccessTarget::Prg, 0) } ; 5];
    // 5117 is 0xFF at start
    prg_selects[4].1 = 0xFF;

    let mut mapper = Self {
      exram: vec![0; 1024].into_boxed_slice(),
      ppu_data_sub: true,

      prg_selects,
      bg_banks,

      ..Default::default()
    };

    mapper.update_prg_and_sram_banks(banks);
    mapper.update_chr_banks(banks);

    Box::new(mapper)
  }

  fn prg_write(&mut self, _: &mut CartBanking, _: usize, _: u8) {}

  fn cart_read(&mut self, addr: usize) -> u8 {
    match addr {
      0x5204 => {
        let irq_pending = self.irq_pending;
        self.irq_pending = false;
        self.irq_requested = None;
        ((irq_pending as u8) << 7) | ((self.ppu_in_frame as u8) << 6)
      },

      0x5025 => 
        (self.multiplicand as u16 * self.multiplier as u16) as u8,
      0x5206 => 
        ((self.multiplicand as u16 * self.multiplier as u16) >> 8) as u8,
      
      0x5C00..=0x5FFF => {
        match self.exram_mode {
          ExRamMode::ReadWrite | ExRamMode::ReadOnly => self.exram[addr - 0x5C00],
          _ => 0xFF,
        }
      }

      _ => 0xFF,
    }
  }

  fn cart_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {    
    match addr {
      0x5100 => {
        self.prg_mode = match val & 0b11 {
          0 => PrgMode::Bank32kb,
          1 => PrgMode::Bank16kb,
          2 => PrgMode::BankMixed,
          _ => PrgMode::Bank8kb,
        };
        self.update_prg_and_sram_banks(banks);
      }
      0x5101 => {
        self.chr_mode = match val & 0b11 {
          0 => ChrMode::Bank8kb,
          1 => ChrMode::Bank4kb,
          2 => ChrMode::Bank2kb,
          _ => ChrMode::Bank1kb,
        };
        self.update_chr_banks(banks);
      }
      0x5102 => self.sram_write_lock1 = val & 0b11 == 0x02,
      0x5103 => self.sram_write_lock2 = val & 0b11 == 0x01,
      0x5104 => self.exram_mode = match val & 0b11 {
        0b00 => ExRamMode::Nametbl,
        0b01 => ExRamMode::NametblEx,
        0b10 => ExRamMode::ReadWrite,
        _    => ExRamMode::ReadOnly,
      },

      0x5105 => {
        for i in 0..4 {
          let bits = (val >> (i*2)) & 0b11;
          self.ciram_mapping[i] = match bits {
            0 => NametblMapping::CiRam0,
            1 => NametblMapping::CiRam1,
            2 => NametblMapping::ExRam,
            _ => NametblMapping::FillMode,
          };

          self.update_ciram_banks(banks);
        }
      }

      0x5106 => self.fill_mode_tile = val,
      0x5107 => self.fill_mode_color = val & 0b11,

      0x5113..=0x5117 => {
        // https://www.nesdev.org/wiki/MMC5#PRG_Bankswitching_($5113-$5117)
        
        let target = match addr {
          0x5113 => AccessTarget::SRam,
          0x5117 => AccessTarget::Prg,
          _ => match (val >> 7) != 0 {
            false => AccessTarget::SRam,
            true  => AccessTarget::Prg,
          },
        };
        
        let mapped = val as usize & 0b0111_1111;
        self.prg_selects[addr - 0x5113] = (target, mapped);
        self.update_prg_and_sram_banks(banks);
      }

      0x5120..=0x512B => {
        // https://www.nesdev.org/wiki/MMC5#CHR_Bankswitching_($5120-$5130)
        
        if !self.ppu_spr_16 && addr > 0x5127 {
          self.last_selected_bg_regs = false;
          return;
        }

        let reg = addr - 0x5120;
        self.chr_selects[reg] = val;
        self.last_selected_bg_regs = addr >= 0x5128;
        self.update_chr_banks(banks);
      }
      0x5130 => self.chr_bank_hi = val & 0b11,

      // 0x5200 => {
      //   self.vsplit_enabled = (val >> 7) != 0;
      //   self.vsplit_region = match (val >> 6) & 1 != 0 {
      //     false => VSplitRegion::Left,
      //     true  => VSplitRegion::Right,
      //   };
      //   self.vsplit_count = val & 0b1_1111;
      // }
      // 0x5201 => self.vsplit_scroll = val,
      // 0x5202 => self.vsplit_bank = val,

      0x5203 => self.irq_value = val,
      0x5204 => {
        self.irq_enabled = (val >> 7) & 1 != 0;

        if self.irq_enabled && self.irq_pending {
          self.irq_requested = Some(());
        } else if !self.irq_enabled {
          self.irq_requested = None;
        }
      }

      0x5205 => self.multiplicand = val,
      0x5206 => self.multiplier = val,

      0x5C00..=0x5FFF => {
        match (&self.exram_mode, self.ppu_in_frame) {
          (ExRamMode::Nametbl | ExRamMode::NametblEx, true) 
          | (ExRamMode::ReadWrite, _) => self.exram[addr - 0x5C00] = val,
          _ => {}
        }
      }
      _ => {}
    }
  }

  fn map_prg_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x4020..=0x5FFF => PrgTarget::Cart,
      0x6000..=0xFFFF => {
        if addr == 0xFFFA || addr == 0xFFFB {
          self.notify_nmi();
        }

        let page = (addr - 0x6000) / 0x2000;
        let (target, _) = self.prg_selects[page];
        match target {
          AccessTarget::Prg => PrgTarget::Prg(banks.prg.translate(addr)),
          AccessTarget::SRam => PrgTarget::SRam(true, banks.sram.translate(addr)),
        }
      }
      _ => unreachable!()
    }
  }

  fn map_ppu_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PpuTarget {
    // if ppu is not rendering and using 8x16 sprites, ppu data is redirected to last chr select
  
    match addr {
      0x0000..=0x1FFF => {
        // https://forums.nesdev.org/viewtopic.php?p=193069#p193069
        let mapped = match (&self.ppu_state, self.in_8x16_mode()) {
          (_, false) => banks.chr.translate(addr),

          (PpuState::FetchBg, true)  => self.bg_banks.translate(addr),
          (PpuState::FetchSpr, true) => banks.chr.translate(addr),
          (PpuState::Vblank, true) => {
            if self.last_selected_bg_regs {
              self.bg_banks.translate(addr)
            } else {
              banks.chr.translate(addr)
            }
          }
        };

        PpuTarget::Chr(mapped)
      },
      0x2000..=0x2FFF => PpuTarget::CiRam(banks.ciram.translate(addr)),
      _ => unreachable!()
    }
  }

  fn notify_ppuctrl(&mut self, val: u8) {
    self.ppu_spr_16 = (val >> 5) != 0;
  }

  fn notify_ppumask(&mut self, val: u8) {
    let data_sub = (val >> 3) & 0b11 != 0;
    
    if !self.ppu_data_sub && data_sub {
      self.notify_nmi();
    } else if !data_sub {
      self.ppu_in_frame = false;
    }

    self.ppu_data_sub = data_sub;
  }

  fn notify_ppu_state(&mut self, state: PpuState) {
    match state {
      PpuState::Vblank => self.ppu_in_frame = false,
      _ => {}
    }

    self.ppu_state = state;
  }

  fn notify_mmc5_scanline(&mut self) {
    // irq is ack when scanline 0 is detected
    // nmi notify when scanline 241 is detected

    if self.ppu_in_frame {
      self.irq_count += 1;

      if self.irq_count == self.irq_value {
        self.irq_pending = true;
        if self.irq_enabled {
          self.irq_requested = Some(());
        }
      }

      if self.irq_count == 241 {
        self.notify_nmi();
      }
    } else {
      self.ppu_in_frame = true;
      self.irq_count = 0;
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}