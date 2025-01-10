// use crate::cart::{CartHeader, Mirroring, PrgTarget, VRamTarget};
// use super::{Banking, ChrBanking, Mapper, PrgBanking, SRamBanking};

// #[derive(Default, serde::Serialize, serde::Deserialize)]
// enum PrgMode { Bank32kb, Bank16kb, BankMixed, #[default] Bank8kb }
// #[derive(Default, serde::Serialize, serde::Deserialize)]
// enum ChrMode { Bank8kb, Bank4kb, Bank2kb, #[default] Bank1kb }
// #[derive(Default, serde::Serialize, serde::Deserialize)]
// enum ExRamMode { Nametbl, NametblEx, ReadWrite, #[default] ReadOnly }
// #[derive(Copy, Clone, Default, serde::Serialize, serde::Deserialize)]
// enum NametblMapping { #[default] CiRam0, CiRam1, ExRam, FillMode }
// #[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
// enum AccessTarget { Prg, SRam }

// // Mapper 5
// // https://www.nesdev.org/wiki/MMC5
// #[derive(serde::Serialize, serde::Deserialize)]
// pub struct MMC5 {
//   ppu_spr_16: bool,
//   ppu_data_sub: bool,
  
//   prg_mode: PrgMode,
//   prg_selects: [(AccessTarget, usize); 5],
//   prg_banks: Banking<PrgBanking>,
//   sram_banks: Banking<SRamBanking>,

//   sram_write_lock1: bool,
//   sram_write_lock2: bool,

//   chr_mode: ChrMode,
//   chr_selects: [u8; 12],
//   chr_banks: Banking<ChrBanking>,
//   last_chr_select: usize,
//   chr_bank_hi: u8,
  
//   exram_mode: ExRamMode,
//   exram: Box<[u8]>,

//   vram_mapping: [NametblMapping; 4],
//   fill_mode_tile: u8,
//   fill_mode_color: u8,

//   irq_enabled: bool,
//   scanline_count: u8,
//   irq_value: u8,
//   irq_scanline: Option<()>,
//   irq_in_frame: bool,

//   mirroring: Mirroring,

//   multiplicand: u8,
//   multiplier: u8,
// }

// impl MMC5 {
//   pub fn update_prg_and_sram_banks(&mut self) {
//     use PrgMode::*;

//     // this is banked here on every mode
//     self.sram_banks.set(0, self.prg_selects[0].1);

//     match self.prg_mode {
//       Bank32kb => {
//         // ignore bit 0,1
//         let prg_bank = self.prg_selects[4].1 << 2;
//         self.prg_banks.set(0, prg_bank);
//         self.prg_banks.set(1, prg_bank+1);
//         self.prg_banks.set(2, prg_bank+2);
//         self.prg_banks.set(3, prg_bank+3);
//       }
//       Bank16kb => {
//         // ignore bit 0
//         let (target, upper_bank) = self.prg_selects[2];
//         match target {
//           AccessTarget::Prg => {
//             self.prg_banks.set(1, upper_bank);
//             self.prg_banks.set(2, upper_bank+1);
//           }
//           AccessTarget::SRam => {
//             self.sram_banks.set(1, upper_bank);
//             self.sram_banks.set(1, upper_bank+1);
//           }
//         }

//         let lower_bank = self.prg_selects[4].1 << 1;
//         self.prg_banks.set(3, lower_bank);
//         self.prg_banks.set(4, lower_bank);
//       }
//       BankMixed => {
//         // ignore bit 0
//         let (target, upper_bank) = self.prg_selects[2];
//         match target {
//           AccessTarget::Prg => {
//             self.prg_banks.set(1, upper_bank);
//             self.prg_banks.set(2, upper_bank+1);
//           }
//           AccessTarget::SRam => {
//             self.sram_banks.set(1, upper_bank);
//             self.sram_banks.set(1, upper_bank+1);
//           }
//         }

//         let (target, lower_bank) = self.prg_selects[3];
//         match target {
//           AccessTarget::Prg => {
//             self.prg_banks.set(2, lower_bank);
//           }
//           AccessTarget::SRam => {
//             self.sram_banks.set(3, lower_bank);
//           }
//         }

//         self.prg_banks.set(4, self.prg_selects[4].1);
//       }
//       Bank8kb => {
//         for (page, (target, bank)) in self.prg_selects.iter()
//           .skip(1).take(3).enumerate()
//         {
//           match target {
//             AccessTarget::Prg  => self.prg_banks.set(page, *bank),
//             AccessTarget::SRam => self.sram_banks.set(page+1, *bank),
//           }
//         }

//         self.prg_banks.set(4, self.prg_selects[4].1);
//       }
//     }
//   }

//   fn update_chr_banks(&mut self, addr: usize) {
//     let higher_regs = addr > 0x5127;

//     use ChrMode::*;
//     match self.chr_mode {
//       Bank8kb => {
//         let reg = if higher_regs { 11 } else { 7 };
//         let bank = self.chr_selects[reg] as usize;
//         for page in 0..8 {
//           self.chr_banks.set(page, bank+page);
//         }
//       }
//       Bank4kb => {
//         match higher_regs {
//           false => {
//             let upper_bank = self.chr_selects[3] as usize;
//             let lower_bank = self.chr_selects[7] as usize;
//             for page in 0..4 {
//               self.chr_banks.set(page, upper_bank+page);
//               self.chr_banks.set(page+4, lower_bank+page);
//             }
//           }
//           true  => {
//             let bank = self.chr_selects[11] as usize;
//             for page in 0..8 {
//               self.chr_banks.set(page, bank+page);
//             }
//           }
//         }
//       }
//       Bank2kb => {
//         let bank = 
//           self.chr_selects[if higher_regs { 9 } else { 1 }] as usize;
//         self.chr_banks.set(0, bank);
//         self.chr_banks.set(1, bank+1);

//         let bank = 
//           self.chr_selects[if higher_regs { 11 } else { 3 }] as usize;
//         self.chr_banks.set(2, bank);
//         self.chr_banks.set(3, bank+1);

//         let bank = 
//           self.chr_selects[if higher_regs { 9 } else { 5 }] as usize;
//         self.chr_banks.set(4, bank);
//         self.chr_banks.set(5, bank+1);
        
//         let bank = 
//           self.chr_selects[if higher_regs { 11 } else { 7 }] as usize;
//         self.chr_banks.set(6, bank);
//         self.chr_banks.set(7, bank+1);
//       }
//       Bank1kb => {
//         let bank = 
//           self.chr_selects[if higher_regs { 8 } else { 0 }] as usize;
//         self.chr_banks.set(0, bank);
//         let bank = 
//           self.chr_selects[if higher_regs { 9 } else { 1 }] as usize;
//         self.chr_banks.set(1, bank);
//         let bank = 
//           self.chr_selects[if higher_regs { 10 } else { 2 }] as usize;
//         self.chr_banks.set(2, bank);
//         let bank = 
//           self.chr_selects[if higher_regs { 11 } else { 3 }] as usize;
//         self.chr_banks.set(3, bank);

//         let bank = 
//           self.chr_selects[if higher_regs { 8 } else { 4 }] as usize;
//         self.chr_banks.set(4, bank);
//         let bank = 
//           self.chr_selects[if higher_regs { 9 } else { 5 }] as usize;
//         self.chr_banks.set(5, bank);
//         let bank = 
//           self.chr_selects[if higher_regs { 10 } else { 6 }] as usize;
//         self.chr_banks.set(6, bank);
//         let bank = 
//           self.chr_selects[if higher_regs { 11 } else { 7 }] as usize;
//         self.chr_banks.set(7, bank);
//       }
//     }
//   }
// }

// #[typetag::serde]
// impl Mapper for MMC5 {
//   fn new(header: &CartHeader) -> Box<Self>  {
//     let prg_banks = Banking::new_prg(header, 4);
//     let chr_banks = Banking::new_chr(header, 8);
//     let sram_banks = Banking::new(header.sram_real_size(), 0x6000, 8*1024, 4);

//     let mut prg_selects = [const { (AccessTarget::Prg, 0) } ; 5];
//     // 5117 is 0xFF at start
//     prg_selects[4].1 = 0xFF;

//     let mapper = Self {
//       exram: vec![0; 1024].into_boxed_slice(),
//       exram_mode: Default::default(),

//       ppu_spr_16: false,
//       ppu_data_sub: true,

//       prg_mode: Default::default(),
//       prg_selects,
//       prg_banks,
//       sram_banks, 
//       sram_write_lock1: false,
//       sram_write_lock2: false,

//       chr_mode: Default::default(),

//       chr_selects: [0; 12],
//       chr_banks,
//       last_chr_select: 0,
//       chr_bank_hi: 0,

//       vram_mapping: [Default::default(); 4],
//       fill_mode_tile: 0,
//       fill_mode_color: 0,

//       irq_enabled: false,
//       scanline_count: 0,
//       irq_value: 0,
//       irq_scanline: None,
//       irq_in_frame: false,

//       mirroring: Default::default(),

//       multiplicand: 0,
//       multiplier: 0,
//     };

//     Box::new(mapper)
//   }

//   fn write(&mut self, _: usize, _: u8) {}

//   fn cart_read(&mut self, addr: usize) -> u8 {
//     match addr {
//       0x5204 => {
//         let irq_ack = self.irq_scanline.take().is_some() as u8;
//         (irq_ack << 7) | ((self.irq_in_frame as u8) << 6)
//       },
//       0x5025 => (self.multiplicand * self.multiplier) & 0x00FF,
//       0x5206 => (((self.multiplicand as u16 * self.multiplier as u16) & 0xFF00) >> 8) as u8,
//       0x5C00..=0x5FFF => {
//         match self.exram_mode {
//           ExRamMode::ReadWrite | ExRamMode::ReadOnly => self.exram[addr - 0x5C00],
//           _ => 0,
//         }
//       }

//       // TODO: open bus behaviour
//       _ => 0,
//     }
//   }

//   fn cart_write(&mut self, addr: usize, val: u8) {
//     match addr {
//       0x5100 => self.prg_mode = match val & 0b11 {
//         0 => PrgMode::Bank32kb,
//         1 => PrgMode::Bank16kb,
//         2 => PrgMode::BankMixed,
//         _ => PrgMode::Bank8kb,
//       },
//       0x5101 => self.chr_mode = match val & 0b11 {
//         0 => ChrMode::Bank8kb,
//         1 => ChrMode::Bank4kb,
//         2 => ChrMode::Bank2kb,
//         _ => ChrMode::Bank1kb,
//       },
//       0x5102 => self.sram_write_lock1 = val & 0b11 == 0x02,
//       0x5103 => self.sram_write_lock2 = val & 0b11 == 0x01,
//       0x5104 => self.exram_mode = match val & 0b11 {
//         0b00 => ExRamMode::Nametbl,
//         0b01 => ExRamMode::NametblEx,
//         0b10 => ExRamMode::ReadWrite,
//         _    => ExRamMode::ReadOnly,
//       },

//       0x5105 => {
//         for i in 0..4 {
//           let bits = (val >> (i*2)) & 0b11;
//           self.vram_mapping[i] = match bits {
//             0 => NametblMapping::CiRam0,
//             1 => NametblMapping::CiRam1,
//             2 => NametblMapping::ExRam,
//             _ => NametblMapping::FillMode,
//           };
//         }
//       }

//       0x5106 => self.fill_mode_tile = val,
//       0x5107 => self.fill_mode_color = val & 0b11,

//       0x5113..=0x5117 => {
//         // https://www.nesdev.org/wiki/MMC5#PRG_Bankswitching_($5113-$5117)

        
//         let target = match addr {
//           0x5113 => AccessTarget::SRam,
//           0x5117 => AccessTarget::Prg,
//           _ => match (val >> 7) != 0 {
//             false => AccessTarget::SRam,
//             true  => AccessTarget::Prg,
//           },
//         };
        
//         let mapped = val as usize & 0b0111_1111;
//         self.prg_selects[addr - 0x5113] = (target, mapped);
//         self.update_prg_and_sram_banks();
//       }

//       0x5120..=0x512B => {
//         // https://www.nesdev.org/wiki/MMC5#CHR_Bankswitching_($5120-$5130)
//         if !self.ppu_spr_16 && addr > 0x5127 {
//           self.last_chr_select = 0;
//           return; 
//         }

//         self.last_chr_select = addr - 0x5120;
//         self.chr_selects[self.last_chr_select] = val;
//         self.update_chr_banks(addr);
//       }
//       0x5130 => self.chr_bank_hi = val & 0b11,

//       // 0x5200 => {
//       //   self.vsplit_enabled = (val >> 7) != 0;
//       //   self.vsplit_region = match (val >> 6) & 1 != 0 {
//       //     false => VSplitRegion::Left,
//       //     true  => VSplitRegion::Right,
//       //   };
//       //   self.vsplit_count = val & 0b1_1111;
//       // }
//       // 0x5201 => self.vsplit_scroll = val,
//       // 0x5202 => self.vsplit_bank = val,

//       0x5203 => self.irq_value = val,
//       0x5204 => {
//         self.irq_enabled = (val >> 7) & 1 != 0;
//         if !self.irq_enabled {
//           self.irq_scanline = None;
//         }
//       }

//       0x5205 => self.multiplicand = val,
//       0x5206 => self.multiplier = val,

//       0x5C00..=0x5FFF => {
//         match (&self.exram_mode, self.irq_in_frame) {
//           (ExRamMode::Nametbl | ExRamMode::NametblEx, true) 
//           | (ExRamMode::ReadWrite, _) => self.exram[addr - 0x5C00] = val,
//           _ => {}
//         }
//       }
//       _ => {}
//     }
//   }

//   fn map_addr(&mut self, addr: usize) -> PrgTarget {
//     match addr { 
//       0x4020..=0x5FFF => PrgTarget::Cart,
//       0x6000..=0xFFFF => {
//         let page = (addr - 0x6000) / (8*1024);
//         let (target, _) = self.prg_selects[page];
//         match target {
//           AccessTarget::SRam => PrgTarget::SRam(true, self.sram_addr(addr)),
//           AccessTarget::Prg => PrgTarget::Prg(self.prg_addr(addr))
//         }
//       }
//       _ => unreachable!()
//     }
//   }

//   fn prg_addr(&mut self, addr: usize) -> usize {
//     self.prg_banks.addr(addr)
//   }

//   fn chr_addr(&mut self, addr: usize) -> usize {
//     self.chr_banks.addr(addr)
//   }

//   fn sram_addr(&mut self, addr: usize) -> usize {
//     self.sram_banks.addr(addr)
//   }

//   fn vram_addr(&mut self, addr: usize) -> VRamTarget {
//     let page = (addr - 0x2000) / 0x400;
//     let target = self.vram_mapping[page];

//     match target {
//       NametblMapping::ExRam => VRamTarget::ExRam(addr % 0x400),
//       NametblMapping::CiRam0 => VRamTarget::CiRam(addr % 0x400),
//       NametblMapping::CiRam1 => VRamTarget::CiRam((addr % 0x400) + 0x400),
//       NametblMapping::FillMode => VRamTarget::ExRam(0),
//     }
//   }

//   // fn vram_read(&mut self, vram: &[u8], addr: usize) -> u8 {
//   //   let addr = addr - 0x2000;
//   //   let page = addr / 0x400;

//   //   let target = self.vram_mapping[page];
//   //   match target {
//   //     NametblMapping::ExRam => self.exram[addr % 0x400],
//   //     NametblMapping::CiRam0 => vram[addr % 0x400],
//   //     NametblMapping::CiRam1 => vram[(addr % 0x400) + 0x400],
//   //     NametblMapping::FillMode => self.fill_mode_tile,
//   //   }
//   // }
//   // fn vram_write(&mut self, vram: &mut[u8], addr: usize, val: u8) {
//   //   let addr = addr - 0x2000;
//   //   let page = addr / 0x400;

//   //   let target = self.vram_mapping[page];
//   //   match target {
//   //     NametblMapping::ExRam => self.exram[addr % 0x400] = val,
//   //     NametblMapping::CiRam0 => vram[addr % 0x400] = val,
//   //     NametblMapping::CiRam1 => vram[(addr % 0x400) + 0x400] = val,
//   //     _ => {}
//   //   }
//   // }

//   fn notify_ppuctrl(&mut self, val: u8) {
//     self.ppu_spr_16 = (val >> 5) != 0;
//   }

//   fn notify_ppumask(&mut self, val: u8) {
//     self.ppu_data_sub = (val >> 3) & 0b11 != 0;
//     if !self.ppu_data_sub {
//       self.irq_in_frame = false;
//     }
//   }

//   // fn notify_in_frame(&mut self, cond: bool) {
//   //   if self.ppu_data_sub {
//   //     self.irq_in_frame = cond;
//   //   }
//   // }

//   fn poll_irq(&mut self) -> bool {
//     self.irq_enabled && self.irq_scanline.is_some()
//   }

//   fn mirroring(&self) -> Mirroring { Mirroring::FourScreen }
// }