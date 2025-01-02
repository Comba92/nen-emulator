use crate::cart::Mirroring;

use super::{Bank, Mapper, DEFAULT_PRG_BANK_SIZE};

// Mapper 7 https://www.nesdev.org/wiki/AxROM
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct AxRom {
    prg_bank_select: Bank,
    mirroring_page: Mirroring,
}

#[typetag::serde]
impl Mapper for AxRom {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE*2 }

    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        self.prg_bank_addr(prg, self.prg_bank_select, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_0111) as usize;
        
        self.mirroring_page = match val & 0b0001_0000 != 0 {
            false   => Mirroring::SingleScreenA,
            true    => Mirroring::SingleScreenB,
        };
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring_page)
    }
}