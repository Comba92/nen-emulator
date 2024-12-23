use super::{Mapper, DEFAULT_PRG_BANK_SIZE};

// Mapper 66 https://www.nesdev.org/wiki/GxROM
#[derive(Default)]
pub struct GxRom {
    prg_bank_select: usize,
    chr_bank_select: usize,
}
impl Mapper for GxRom {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE*2 }

    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        self.prg_bank_addr(prg, self.prg_bank_select, addr)
    }

    fn chr_addr(&self, chr: &[u8], addr: usize) -> usize {
        self.chr_bank_addr(chr, self.chr_bank_select, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize;
        self.prg_bank_select = ((val & 0b0011_0000) >> 4) as usize;
    }
}