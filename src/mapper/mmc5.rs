use super::{Banking, Mapper};
use crate::{
  apu::{pulse::Pulse, Channel},
  banks::{ChrBanking, MemConfig},
  cart::CartHeader,
  mem::{self, MemMapping},
  ppu::RenderingState,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, PartialEq)]
enum PrgMode {
  Bank32kb,
  Bank16kb,
  BankMixed,
  #[default]
  Bank8kb,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Debug)]
enum ChrMode {
  Bank8kb,
  Bank4kb,
  Bank2kb,
  #[default]
  Bank1kb,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, PartialEq)]
enum ExRamMode {
  Nametbl,
  NametblEx,
  CpuReadWrite,
  #[default]
  CpuReadOnly,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, Clone, Default, Debug)]
enum NametblMapping {
  #[default]
  CiRam0,
  CiRam1,
  ExRam,
  FillMode,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Clone, Copy)]
enum AccessTarget {
  #[default]
  Prg,
  SRam,
}

// Mapper 5
// https://www.nesdev.org/wiki/MMC5
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct MMC5 {
  ppu_spr_16: bool,
  ppu_data_sub: bool,
  ppu_state: RenderingState,

  prg_mode: PrgMode,
  prg_selects: [(AccessTarget, usize); 5],

  sram_write_lock1: bool,
  sram_write_lock2: bool,

  chr_mode: ChrMode,
  chr_selects: [u8; 12],
  spr_banks: Banking<ChrBanking>,
  bg_banks: Banking<ChrBanking>,
  // spr_banks: Banking,
  // bg_banks:  Banking,
  last_selected_bg_regs: bool,
  chr_select_hi: u8,

  exram_mode: ExRamMode,
  exram: Box<[u8]>,
  ex_attr_bank: Banking<ChrBanking>,
  last_nametbl_addr: usize,

  nametbls_mapping: [NametblMapping; 4],
  fill_mode_tile_id: u8,
  fill_mode_palette_id: u8,

  irq_enabled: bool,
  irq_pending: bool,
  irq_value: u8,
  irq_count: u8,
  irq_requested: Option<()>,
  ppu_in_frame: bool,

  multiplicand: u8,
  multiplier: u8,

  pulse1: Pulse,
  pulse2: Pulse,
  cycles: usize,
}

// https://github.com/SourMesen/Mesen2/blob/master/Core/NES/Mappers/Nintendo/MMC5.h
impl MMC5 {
  fn notify_nmi(&mut self) {
    self.ppu_in_frame = false;
    self.irq_pending = false;
    self.irq_requested = None;
    self.irq_count = 0;
  }

  fn set_prg_page(&self, banks: &mut MemConfig, reg: usize, page: usize) {
    let (target, bank) = self.prg_selects[reg];
    match target {
      AccessTarget::Prg => banks.prg.set_page(page, bank),
      AccessTarget::SRam => banks.sram.set_page(page + 1, bank),
    }
  }

  fn set_prg_page2(&self, banks: &mut MemConfig, reg: usize, page: usize) {
    let (target, bank) = self.prg_selects[reg];
    let bank = bank & !1;
    match target {
      AccessTarget::Prg => {
        banks.prg.set_page(page, bank);
        banks.prg.set_page(page + 1, bank | 1);
      }
      AccessTarget::SRam => {
        banks.sram.set_page(page + 1, bank);
        banks.sram.set_page(page + 2, bank | 1);
      }
    }
  }

  fn update_prg_and_sram_banks(&mut self, banks: &mut MemConfig) {
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

    for (i, (target, _)) in self.prg_selects.iter().enumerate() {
      let handler = MemMapping::SRAM_HANDLER + i;
      match target {
        AccessTarget::Prg => {
          banks.mapping.cpu_reads[handler] = mem::prg_read;
          banks.mapping.cpu_writes[handler] = mem::prg_write;
        }
        AccessTarget::SRam => {
          banks.mapping.cpu_reads[handler] = mem::sram_read;
          banks.mapping.cpu_writes[handler] = mem::sram_write;
        }
      }
    }
  }

  fn update_spr_banks(&mut self) {
    match self.chr_mode {
      ChrMode::Bank8kb => {
        let bank = self.chr_selects[7] as usize;
        for page in 0..8 {
          self.spr_banks.set_page(page, bank + page);
        }
      }
      ChrMode::Bank4kb => {
        let bank = self.chr_selects[4] as usize;
        for page in 0..4 {
          self.spr_banks.set_page(page, bank + page);
        }

        let bank = self.chr_selects[7] as usize;
        for page in 4..8 {
          self.spr_banks.set_page(page, bank + (page - 4));
        }
      }
      ChrMode::Bank2kb => {
        let bank = self.chr_selects[1] as usize;
        self.spr_banks.set_page(0, bank);
        self.spr_banks.set_page(1, bank + 1);

        let bank = self.chr_selects[3] as usize;
        self.spr_banks.set_page(2, bank);
        self.spr_banks.set_page(3, bank + 1);

        let bank = self.chr_selects[5] as usize;
        self.spr_banks.set_page(4, bank);
        self.spr_banks.set_page(5, bank + 1);

        let bank = self.chr_selects[7] as usize;
        self.spr_banks.set_page(6, bank);
        self.spr_banks.set_page(7, bank + 1);
      }
      ChrMode::Bank1kb => {
        for i in 0..8 {
          self.spr_banks.set_page(i, self.chr_selects[i] as usize);
        }
      }
    }
  }

  fn update_bg_banks(&mut self) {
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
          self.bg_banks.set_page(page, bank + (page - 4));
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
          self.bg_banks
            .set_page(i, self.chr_selects[8 + (i % 4)] as usize);
        }
      }
    }
  }

  // fn ex_attribute_val(&mut self, addr: usize) -> PpuTarget {
  //   // https://www.nesdev.org/wiki/MMC5#Extended_attributes

  //   if is_attribute(addr - 0x2000) {
  //   self.last_nametbl_addr = addr;
  //   let ex_attribute = self.exram_read(addr - 0x2000);
  //   let pal = ex_attribute >> 6;
  //   let attribute = (pal << 6) | (pal << 4) | (pal << 2) | pal;
  //   PpuTarget::Value(attribute)
  //   } else {
  //   let ex_attribute = self.exram_read(self.last_nametbl_addr - 0x2000);
  //   let bank = ((self.chr_select_hi as usize) << 6) | (ex_attribute as usize & 0b0011_1111);
  //   let mapped = (bank << 12) + (addr & 0xFFF);
  //   PpuTarget::Chr(mapped)
  //   }
  // }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for MMC5 {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    let spr_banks = Banking::new_chr(header, 8);
    let bg_banks = Banking::new_chr(header, 8);
    let ex_attr_bank = Banking::new(header.chr_real_size(), 0, 4 * 1024, 1);
    banks.sram = Banking::new(header.sram_real_size(), 0x6000, 8 * 1024, 4);

    let mut mapper = Self {
      exram: vec![0; 1024].into_boxed_slice(),
      ppu_data_sub: true,
      spr_banks,
      bg_banks,
      ex_attr_bank,

      ..Default::default()
    };

    // 5117 is 0xFF at start
    mapper.prg_selects[4].1 = 0xFF;

    mapper.update_prg_and_sram_banks(banks);
    mapper.update_spr_banks();
    mapper.update_bg_banks();

    Box::new(mapper)
  }

  fn prg_write(&mut self, _: &mut MemConfig, _: usize, _: u8) {}

  fn cart_read(&mut self, addr: usize) -> u8 {
    match addr {
      0x5015 => ((self.pulse2.is_enabled() as u8) << 1) | self.pulse1.is_enabled() as u8,

      0x5204 => {
        let irq_pending = self.irq_pending;
        self.irq_pending = false;
        self.irq_requested = None;
        ((irq_pending as u8) << 7) | ((self.ppu_in_frame as u8) << 6)
      }

      0x5025 => (self.multiplicand as u16 * self.multiplier as u16) as u8,
      0x5206 => ((self.multiplicand as u16 * self.multiplier as u16) >> 8) as u8,

      0x5C00..=0x5FFF => match self.exram_mode {
        ExRamMode::CpuReadWrite | ExRamMode::CpuReadOnly => self.exram_read(addr - 0x5C00),
        _ => 0xFF,
      },

      _ => 0xFF,
    }
  }

  fn cart_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x5000 => self.pulse1.set_ctrl(val),
      0x5004 => self.pulse2.set_ctrl(val),

      0x5002 => self.pulse1.set_timer_low(val),
      0x5006 => self.pulse2.set_timer_low(val),

      0x5003 => self.pulse1.set_timer_high(val),
      0x5007 => self.pulse2.set_timer_high(val),

      0x5015 => {
        self.pulse1.set_enabled(val & 0b0001 != 0);
        self.pulse2.set_enabled(val & 0b0010 != 0);
      }

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

        if self.last_selected_bg_regs {
          self.update_bg_banks();
        } else {
          self.update_spr_banks();
        }
      }
      0x5102 => self.sram_write_lock1 = val & 0b11 == 0x02,
      0x5103 => self.sram_write_lock2 = val & 0b11 == 0x01,
      0x5104 => {
        self.exram_mode = match val & 0b11 {
          0b00 => ExRamMode::Nametbl,
          0b01 => ExRamMode::NametblEx,
          0b10 => ExRamMode::CpuReadWrite,
          _ => ExRamMode::CpuReadOnly,
        };
      }

      0x5105 => {
        for i in 0..4 {
          let bits = (val >> (i * 2)) & 0b11;
          self.nametbls_mapping[i] = match bits {
            0 => {
              banks.vram.set_page(i, 0);
              NametblMapping::CiRam0
            }
            1 => {
              banks.vram.set_page(i, 1);
              NametblMapping::CiRam1
            }
            2 => NametblMapping::ExRam,
            _ => NametblMapping::FillMode,
          };
        }
      }

      0x5106 => self.fill_mode_tile_id = val,
      0x5107 => self.fill_mode_palette_id = val & 0b11,

      0x5113..=0x5117 => {
        // https://www.nesdev.org/wiki/MMC5#PRG_Bankswitching_($5113-$5117)

        let target = match addr {
          0x5113 => AccessTarget::SRam,
          0x5117 => AccessTarget::Prg,
          _ => match (val >> 7) != 0 {
            false => AccessTarget::SRam,
            true => AccessTarget::Prg,
          },
        };

        let mapped = match target {
          AccessTarget::Prg => val as usize & 0b0111_1111,
          AccessTarget::SRam => val as usize & 0b0000_1111,
        };

        self.prg_selects[addr - 0x5113] = (target, mapped);
        self.update_prg_and_sram_banks(banks);
      }

      // https://www.nesdev.org/wiki/MMC5#CHR_Bankswitching_($5120-$5130)
      0x5120..=0x5127 => {
        let reg = addr - 0x5120;
        self.chr_selects[reg] = val;
        self.last_selected_bg_regs = false;

        self.update_spr_banks();
      }
      0x5128..=0x512B => {
        let reg = addr - 0x5120;
        self.chr_selects[reg] = val;
        self.last_selected_bg_regs = self.ppu_spr_16;

        self.update_bg_banks();
      }
      0x5130 => self.chr_select_hi = val & 0b11,

      // 0x5200 => {
      //   self.vsplit_enabled = (val >> 7) != 0;
      //   self.vsplit_region = match (val >> 6) & 1 != 0 {
      //   false => VSplitRegion::Left,
      //   true  => VSplitRegion::Right,
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

      0x5C00..=0x5FFF => match (&self.exram_mode, self.ppu_in_frame) {
        (ExRamMode::Nametbl | ExRamMode::NametblEx, true)
        | (ExRamMode::CpuReadWrite, _) => self.exram_write(addr - 0x5C00, val),
        _ => {}
      },
      _ => {}
    }
  }

  // fn map_prg_addr_branching(&mut self, banks: &mut MemConfig, addr: usize) -> PrgTarget {
  //   match addr {
  //   0x4020..=0x5FFF => PrgTarget::Cart,
  //   0x6000..=0xFFFF => {
  //     if addr == 0xFFFA || addr == 0xFFFB {
  //     self.notify_nmi();
  //     }

  //     let page = (addr - 0x6000) / 0x2000;
  //     let (target, _) = self.prg_selects[page];
  //     match target {
  //     AccessTarget::Prg => PrgTarget::Prg(banks.prg.translate(addr)),
  //     AccessTarget::SRam => PrgTarget::SRam(true, banks.sram.translate(addr)),
  //     }
  //   }
  //   _ => unreachable!()
  //   }
  // }

  fn prg_translate(&mut self, banks: &mut MemConfig, addr: u16) -> usize {
    if addr == 0xFFFA || addr == 0xFFFB {
      self.notify_nmi();
    }

    banks.prg.translate(addr as usize)
  }

  fn chr_translate(&mut self, banks: &mut MemConfig, addr: u16) -> usize {
    let addr = addr as usize;

    if self.exram_mode == ExRamMode::NametblEx
      && self.ppu_data_sub
      && self.ppu_state == RenderingState::FetchBg
    {
      let ex_attribute = self.exram_read(self.last_nametbl_addr - 0x2000);
      let bank = ((self.chr_select_hi as usize) << 6) | (ex_attribute as usize & 0b0011_1111);
      let mapped = (bank << 12) + (addr & 0xFFF);
      mapped % banks.chr.data_size
    } else {
      // https://forums.nesdev.org/viewtopic.php?p=193069#p193069
      let mapped = match (&self.ppu_state, self.ppu_spr_16 && self.ppu_data_sub) {
        (_, false) => self.spr_banks.translate(addr),

        (RenderingState::FetchBg, true) => self.bg_banks.translate(addr),
        (RenderingState::FetchSpr, true) => self.spr_banks.translate(addr),
        (RenderingState::Vblank, true) => {
          if self.last_selected_bg_regs {
            self.bg_banks.translate(addr)
          } else {
            self.spr_banks.translate(addr)
          }
        }
      };

      mapped
    }
  }

  // fn map_ppu_addr_branching(&mut self, banks: &mut MemConfig, addr: usize) -> PpuTarget {
  //   match addr {
  //   0x0000..=0x1FFF => {
  //     if self.exram_mode == ExRamMode::NametblEx && self.ppu_data_sub && self.ppu_state == RenderingState::FetchBg {
  //     let ex_attribute = self.exram_read(self.last_nametbl_addr - 0x2000);
  //     let bank = ((self.chr_select_hi as usize) << 6) | (ex_attribute as usize & 0b0011_1111);
  //     let mapped = (bank << 12) + (addr & 0xFFF);
  //     PpuTarget::Chr(mapped % banks.chr.data_size)
  //     } else {
  //     // https://forums.nesdev.org/viewtopic.php?p=193069#p193069
  //     let mapped = match (&self.ppu_state, self.ppu_spr_16 && self.ppu_data_sub) {
  //       (_, false) => self.spr_banks.translate(addr),

  //       (RenderingState::FetchBg, true)  => self.bg_banks.translate(addr),
  //       (RenderingState::FetchSpr, true) => self.spr_banks.translate(addr),
  //       (RenderingState::Vblank, true) => {
  //       if self.last_selected_bg_regs {
  //         self.bg_banks.translate(addr)
  //       } else {
  //         self.spr_banks.translate(addr)
  //       }
  //       }
  //     };

  //     PpuTarget::Chr(mapped)
  //     }
  //   },

  //   0x2000..=0x2FFF => {
  //     if self.exram_mode == ExRamMode::NametblEx && self.ppu_data_sub {
  //     if is_attribute(addr - 0x2000) {
  //       let ex_attribute = self.exram_read(self.last_nametbl_addr - 0x2000);
  //       let pal = ex_attribute >> 6;
  //       let attribute = (pal << 6) | (pal << 4) | (pal << 2) | pal;
  //       return PpuTarget::Value(attribute);
  //     } else {
  //       self.last_nametbl_addr = addr;
  //     }
  //     }

  //     let page = (addr - 0x2000) / 1024;
  //     let target = self.nametbls_mapping[page];

  //     match target {
  //     NametblMapping::CiRam0 | NametblMapping::CiRam1
  //       => PpuTarget::CiRam(banks.ciram.translate(addr)),

  //     NametblMapping::ExRam => {
  //       match self.exram_mode {
  //       ExRamMode::Nametbl | ExRamMode::NametblEx
  //         => PpuTarget::ExRam(addr - 0x2000),
  //       _ => PpuTarget::Value(0),
  //       }
  //     }

  //     NametblMapping::FillMode => {
  //       match is_attribute(addr - 0x2000) {
  //       false => PpuTarget::Value(self.fill_mode_tile_id),
  //       true  => {
  //         let pal = self.fill_mode_palette_id;
  //         let attribute = (pal << 6) | (pal << 4) | (pal << 2) | pal;
  //         PpuTarget::Value(attribute)
  //       }
  //       }
  //     },
  //     }
  //   }

  //   _ => unreachable!()
  //   }
  // }

  fn exram_read(&mut self, addr: usize) -> u8 {
    self.exram[addr % self.exram.len()]
  }

  fn exram_write(&mut self, addr: usize, val: u8) {
    self.exram[addr % self.exram.len()] = val;
  }

  fn notify_ppuctrl(&mut self, val: u8) {
    self.ppu_spr_16 = (val >> 5) & 0b1 == 1;
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

  fn notify_ppu_state(&mut self, state: RenderingState) {
    match state {
      RenderingState::Vblank => self.notify_nmi(),
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
    } else {
      self.irq_requested = None;
      self.irq_pending = false;
      self.ppu_in_frame = true;
      self.irq_count = 0;
    }
  }

  fn notify_cpu_cycle(&mut self) {
    if self.cycles % 2 == 1 {
      self.pulse1.step_timer();
      self.pulse2.step_timer();
    }

    // envelope and length counter are fixed to a 240hz update rate
    if self.cycles >= 7457 {
      self.cycles = 0;
      self.pulse1.step_quarter();
      self.pulse1.step_half();
      self.pulse2.step_quarter();
      self.pulse2.step_half();
    } else {
      self.cycles += 1;
    }
  }

  fn get_sample(&self) -> u8 {
    self.pulse1.get_sample() + self.pulse2.get_sample()
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}
