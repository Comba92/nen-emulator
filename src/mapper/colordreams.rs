use super::{Bank, Mapper, DEFAULT_PRG_BANK_SIZE};

// Mapper 11 https://www.nesdev.org/wiki/Color_Dreams
#[derive(Default)]
pub struct ColorDreams {
    prg_bank_select: Bank,
    chr_bank_select: Bank,
}
impl Mapper for ColorDreams {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE*2 }

    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        self.prg_bank_addr(prg, self.prg_bank_select, addr)
    }

    fn chr_addr(&self, chr: &[u8], addr: usize) -> usize {
        self.chr_bank_addr(chr, self.chr_bank_select, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_0011) as usize;
        self.chr_bank_select = ((val & 0b1111_0000) >> 4) as usize;
    }
}