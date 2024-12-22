use super::{Mapper, DEFAULT_PRG_BANK_SIZE};

// Mapper 66 https://www.nesdev.org/wiki/GxROM
#[derive(Default)]
pub struct GxRom {
    pub prg_bank_select: usize,
    pub chr_bank_select: usize,
}
impl Mapper for GxRom {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE*2 }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        self.read_prg_bank(prg, self.prg_bank_select, addr)
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        self.read_chr_bank(chr, self.chr_bank_select, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize;
        self.prg_bank_select = ((val & 0b0011_0000) >> 4) as usize;
    }
}