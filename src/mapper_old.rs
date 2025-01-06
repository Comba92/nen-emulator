// mod nrom;
// mod mmc1;
// mod uxrom;
// mod inesmapper3;
// mod mmc3;
// mod axrom;
// mod mmc2;
// mod colordreams;
// mod gxrom;
// mod vrc2_4;
// mod inesmapper71;
// mod mmc5;

// use axrom::AxRom;
// use colordreams::ColorDreams;
// use gxrom::GxRom;
// use inesmapper3::INesMapper003;
// use inesmapper71::INesMapper071;
// use mmc1::Mmc1;
// use mmc2::Mmc2;
// use mmc3::Mmc3;
// // use mmc5::Mmc5;
// use nrom::NRom;
// use uxrom::UxRom;
// use vrc2_4::Vrc2_4;

// use crate::cart::Mirroring;

// #[typetag::serde(tag = "type")]
// pub trait Mapper {
//     fn prg_addr(&self, _prg: &[u8], addr: usize) -> usize { addr - ROM_START }
//     fn chr_addr(&self, _chr: &[u8], addr: usize) -> usize { addr }

//     // TODO: open bus behaviour
//     fn prg_read(&mut self, prg: &[u8], addr: usize) -> u8 { prg[self.prg_addr(prg, addr)] }
//     fn chr_read(&mut self, chr: &[u8], addr: usize) -> u8 { chr[self.chr_addr(chr, addr)] }

//     // TODO: open bus behaviour
//     fn cart_read(&mut self, _addr: usize) -> u8 { 0 }
//     fn cart_write(&mut self, _addr: usize, _val: u8) {}
//     fn prg_write(&mut self, prg: &mut[u8], addr: usize, val: u8);
//     fn chr_write(&mut self, chr: &mut[u8], addr: usize, val: u8) { chr[self.chr_addr(chr, addr)] = val; }

//     fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE }
//     fn chr_bank_size(&self) -> usize { DEFAULT_CHR_BANK_SIZE }

//     fn prg_banks_count(&self, prg: &[u8]) -> usize { prg.len() / self.prg_bank_size() }
//     fn chr_banks_count(&self, chr: &[u8]) -> usize { chr.len() / self.chr_bank_size() }
    
//     fn prg_last_bank(&self, prg: &[u8]) -> Bank { self.prg_banks_count(prg) - 1 }

//     fn prg_bank_addr(&self, prg: &[u8], bank: Bank, addr: usize) -> usize {
//         let bank_start = (bank % self.prg_banks_count(prg)) * self.prg_bank_size();
//         let offset = (addr - ROM_START) % self.prg_bank_size();
//         bank_start + offset
//     }

//     fn chr_bank_addr(&self, chr: &[u8], bank: Bank, addr: usize) -> usize {
//         let bank_start = (bank % self.chr_banks_count(chr)) * self.chr_bank_size();
//         let offset = addr % self.chr_bank_size();
//         bank_start + offset
//     }

//     fn mirroring(&self) -> Option<Mirroring> { None }
//     fn get_sram(&self) -> Option<&[u8]> { None }
//     fn set_sram(&mut self, _bytes: &[u8]) {}

//     // Mmc3 scanline notify
//     fn notify_scanline(&mut self) {}

//     // Generic cpu cycle notify
//     fn notify_cpu_cycle(&mut self) {}

//     // Mmc5 ppu notify
//     fn notify_ppuctrl(&mut self, _val: u8) {}
//     fn notify_ppumask(&mut self, _val: u8) {}

//     fn poll_irq(&mut self) -> bool { false }
// }

// pub type CartMapper = Box<dyn Mapper>;
// pub fn new_mapper(mapper: u16, _submapper: u8, sram_size: usize) -> Result<CartMapper, String> {
//     let mapper: CartMapper = match mapper {
//         0  => Box::new(NRom),
//         1  => Box::new(Mmc1::new(sram_size)),
//         2  => Box::new(UxRom::default()),
//         3  => Box::new(INesMapper003::default()),
//         4  => Box::new(Mmc3::default()),
//         // 5  => Box::new(Mmc5::new(sram_size)),
//         7  => Box::new(AxRom::default()),
//         9  => Box::new(Mmc2::default()),
//         11 => Box::new(ColorDreams::default()),
//         // 16 => // TODO, Dragon Ball games
//         // https://www.nesdev.org/wiki/INES_Mapper_016
//         21 | 22 | 23 | 25 => Box::new(Vrc2_4::new(mapper)),
//         66 => Box::new(GxRom::default()),
//         // 69 => // TODO, this only plays Batman: Return of the Joker
//         // https://www.nesdev.org/wiki/Sunsoft_FME-7
//         71 => Box::new(INesMapper071::default()),
//         _ => return Err(format!("Mapper {mapper} not implemented"))
//     };

//     Ok(mapper)
// }

// pub fn mapper_name(id: u16) -> &'static str {
//     MAPPERS_TABLE.iter()
//       .find(|m| m.0 == id)
//       .map(|m| m.1)
//       .unwrap_or("Not implemented")
// }
// const MAPPERS_TABLE: [(u16, &'static str); 16] = [
//     (0, "NRom"),
//     (1, "MMC1"),
//     (2, "UxRom"),
//     (3, "CNRom (INesMapper003)"),
//     (4, "MMC3"),
//     (5, "MMC5"),
//     (7, "AxRom"),
//     (9, "MMC2"),
//     (11, "ColorDreams"),
//     (21, "VRC2/VRC4"),
//     (22, "VRC2/VRC4"),
//     (23, "VRC2/VRC4"),
//     (25, "VRC2/VRC4"),
//     (66, "GxRom"),
//     (69, "Sunsoft FME-7"),
//     (71, "Codemasters (INesMapper071)"),
// ];
// const SRAM_START: usize = 0x6000;
// const ROM_START: usize  = 0x8000;
// const DEFAULT_PRG_BANK_SIZE: usize = 16*1024; // 16 KiB
// const DEFAULT_CHR_BANK_SIZE: usize = 8*1024; // 8 KiB
// pub(self) type Bank = usize;

// #[derive(serde::Serialize, serde::Deserialize)]
// pub struct Dummy;
// #[typetag::serde]
// impl Mapper for Dummy {
//     fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}
//     fn prg_read(&mut self, _prg: &[u8], _addr: usize) -> u8 { 0 }
//     fn chr_read(&mut self, _chr: &[u8], _addr: usize) -> u8 { 0 }
// }

// // #[derive(Debug)]
// // struct BanksConfig {
// //     prg_banks: Vec<usize>,
// //     prg_bank_size: usize,
// //     prg_banks_count: usize,
// //     chr_banks: Vec<usize>,
// //     chr_bank_size: usize,
// //     chr_banks_count: usize,
// // }
// // impl BanksConfig {
// //     pub fn new(
// //         prg_bank_size: usize,
// //         chr_bank_size: usize,
// //     ) -> Self {
// //         let mut prg_banks = Vec::new();
// //         for bank_start in 0..(32*1024)/prg_bank_size {
// //             prg_banks.push(0x8000 + bank_start*prg_bank_size);
// //         }

// //         let mut chr_banks = Vec::new();
// //         for bank_start in 0..(8*1024)/chr_bank_size {
// //             chr_banks.push(bank_start*chr_bank_size);
// //         }

// //         Self { 
// //             prg_banks_count: prg_banks.len(),
// //             chr_banks_count: chr_banks.len(),
// //             prg_banks,
// //             prg_bank_size,
// //             chr_banks,
// //             chr_bank_size,
// //         }
// //     }

// //     pub fn prg_switch(&mut self, bank_old: usize, bank_new: usize) {
// //         let tmp = self.prg_banks[bank_old];
// //         self.prg_banks[bank_old] = self.prg_banks[bank_new];
// //         self.prg_banks[bank_new] = tmp;
// //     }

// //     pub fn chr_switch(&mut self, bank_old: usize, bank_new: usize) {
// //         let tmp = self.chr_banks[bank_old];
// //         self.chr_banks[bank_old] = self.chr_banks[bank_new];
// //         self.chr_banks[bank_new] = tmp;
// //     }

// //     pub fn prg_addr(&self, addr: usize) -> usize {
// //         let window = (addr - 0x8000) / self.prg_bank_size;
// //         let bank = self.prg_banks[window] * self.prg_bank_size;
// //         let offset = addr % self.prg_bank_size;
// //         bank + offset
// //     }

// //     pub fn chr_addr(&self, addr: usize) -> usize {
// //         let window = addr / self.chr_bank_size;
// //         let bank = self.chr_banks[window] * self.chr_bank_size;
// //         let offset = addr % self.chr_bank_size;
// //         bank + offset
// //     }
// // }


// // #[cfg(test)]
// // mod banking_tests {
// //     use super::BanksConfig;

// //     #[test]
// //     fn banking() {
// //         let nrom = BanksConfig::new(
// //             16*1024,
// //             8 * 1024,
// //         );
// //         println!("NROM {:X?}", nrom);

// //         let uxrom = BanksConfig::new(
// //             16*1024,
// //             8*1024,
// //         );
// //         println!("UxROM {:x?}", uxrom);

// //         let mmc1 = BanksConfig::new(
// //             16*1024,
// //             4*1024,
// //         );
// //         println!("MMC1 {:x?}", mmc1);
// //         let addr = mmc1.prg_addr(0xC060);
// //         println!("{addr}");

// //         let cnrom = BanksConfig::new(
// //             16*1024,
// //             8*1024,
// //         );
// //         println!("CNROM {:x?}", cnrom);

// //         let mmc3 = BanksConfig::new(
// //             8*1024,
// //             1*1024,
// //         );
// //         println!("MMC3 {:x?}", mmc3);

// //         let axrom = BanksConfig::new(
// //             32*1024,
// //             8*1024,
// //         );
// //         println!("AxROM {:x?}", axrom);

// //         let gxrom = BanksConfig::new(
// //             32*1024,
// //             8*1024,
// //         );
// //         println!("GxROM {:x?}", gxrom);

// //         let mmc2 = BanksConfig::new(
// //             8*1024, 4*1024
// //         );
// //         println!("MMC2 {:x?}", mmc2);
// //     }
// // }