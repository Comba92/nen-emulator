use crate::cart::Mirroring;

use super::{Bank, Mapper, DEFAULT_CHR_BANK_SIZE, DEFAULT_PRG_BANK_SIZE};

#[derive(Default, Clone, Copy)]
enum Latch { FD, #[default] FE }

// Mapper 9 https://www.nesdev.org/wiki/MMC2
#[derive(Default)]
pub struct Mmc2 {
    prg_bank_select: Bank,
    chr_bank0_select: [Bank; 2],
    chr_bank1_select: [Bank; 2],
    latch: [Latch; 2],
    mirroring: Mirroring,
}
impl Mapper for Mmc2 {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE/2 }
    fn chr_bank_size(&self) -> usize { DEFAULT_CHR_BANK_SIZE/2 }

    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        let bank = match addr {
            0x8000..=0x9FFF => self.prg_bank_select,
            0xA000..=0xBFFF => self.prg_last_bank(prg)-2,
            0xC000..=0xDFFF => self.prg_last_bank(prg)-1,
            0xE000..=0xFFFF => self.prg_last_bank(prg),
            _ => unreachable!()
        };

        self.prg_bank_addr(prg, bank, addr)
    }

    fn chr_addr(&self, chr: &[u8], addr: usize) -> usize {
        let bank = match addr {
            0x0000..=0x0FFF => self.chr_bank0_select[self.latch[0] as usize],
            0x1000..=0x1FFF => self.chr_bank1_select[self.latch[1] as usize],
            _ => unreachable!()
        };

        self.chr_bank_addr(chr, bank, addr)
    }

    fn chr_read(&mut self, chr: &[u8], addr: usize) -> u8 {
        let mapped_addr = self.chr_addr(chr, addr);

        match addr {
            0x0FD8 => self.latch[0] = Latch::FD,
            0x0FE8 => self.latch[0] = Latch::FE,
            0x1FD8..=0x1FDF => self.latch[1] = Latch::FD,
            0x1FE8..=0x1FEF => self.latch[1] = Latch::FE,
            _ => {}
        };

        chr[mapped_addr]
    }

    fn prg_write(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
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
                    0 => Mirroring::Vertical,
                    1 => Mirroring::Horizontal,
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