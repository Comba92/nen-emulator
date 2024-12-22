use crate::cart::Mirroring;

use super::{Mapper, DEFAULT_PRG_BANK_SIZE};

// Mapper 7 https://www.nesdev.org/wiki/AxROM
#[derive(Default)]
pub struct AxRom {
    pub prg_bank_select: usize,
    pub mirroring_page: Mirroring,
}

impl Mapper for AxRom {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE*2 }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        self.read_prg_bank(prg, self.prg_bank_select, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_0111) as usize;
        
        self.mirroring_page = match val & 0b0001_0000 != 0 {
            false   => Mirroring::SingleScreenFirstPage,
            true    => Mirroring::SingleScreenSecondPage,
        };
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring_page)
    }
}