use crate::cart::Mirroring;

use super::{Mapper, DEFAULT_CHR_BANK_SIZE, DEFAULT_PRG_BANK_SIZE};

#[derive(Default, Clone, Copy)]
enum Latch { FD, #[default] FE }

// Mapper 9 https://www.nesdev.org/wiki/MMC2
#[derive(Default)]
pub struct Mmc2 {
    pub prg_bank_select: usize,
    pub chr_bank0_select: [usize; 2],
    pub chr_bank1_select: [usize; 2],
    pub latch: [Latch; 2],
    pub mirroring: Mirroring,
}
impl Mapper for Mmc2 {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE/2 }
    fn chr_bank_size(&self) -> usize { DEFAULT_CHR_BANK_SIZE/2 }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        let bank = match addr {
            0x8000..=0x9FFF => self.prg_bank_select,
            0xA000..=0xBFFF => self.last_prg_bank(prg)-2,
            0xC000..=0xDFFF => self.last_prg_bank(prg)-1,
            0xE000..=0xFFFF => self.last_prg_bank(prg),
            _ => unreachable!()
        };

        self.read_prg_bank(prg, bank, addr)
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        let bank = match addr {
            0x0000..=0x0FFF => self.chr_bank0_select[self.latch[0] as usize],
            0x1000..=0x1FFF => self.chr_bank1_select[self.latch[1] as usize],
            _ => unreachable!()
        };
        
        match addr {
            0x0FD8 => self.latch[0] = Latch::FD,
            0x0FE8 => self.latch[0] = Latch::FE,
            0x1FD8..=0x1FDF => self.latch[1] = Latch::FD,
            0x1FE8..=0x1FEF => self.latch[1] = Latch::FE,
            _ => {}
        }

        self.read_chr_bank(chr, bank, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        let val = val as usize & 0b1_1111;

        match addr {
            0xA000..=0xAFFF => self.prg_bank_select = val & 0b1111,
            0xB000..=0xBFFF => {
                self.chr_bank0_select[0] = val;
            }
            0xC000..=0xCFFF => {
                self.chr_bank0_select[1] = val;
            }
            0xD000..=0xDFFF => {
                self.chr_bank1_select[0] = val;
            }
            0xE000..=0xEFFF => {
                self.chr_bank1_select[1] = val;
            }
            0xF000..=0xFFFF => {
                self.mirroring = match val & 1 {
                    0 => Mirroring::Vertically,
                    1 => Mirroring::Horizontally,
                    _ => unreachable!()
                };
            }
            _ => unreachable!()
        }
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }
}