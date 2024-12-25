use super::{Bank, Mapper, ROM_START};

// Mapper 3 https://www.nesdev.org/wiki/INES_Mapper_003
// https://www.nesdev.org/wiki/CNROM

#[derive(Default)]
pub struct INesMapper003 {
    chr_bank_select: Bank,
}
impl Mapper for INesMapper003 {
    // Same as NROM
    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        // if it only has 16KiB, then mirror to first bank
        if prg.len() == self.prg_bank_size() { 
            self.prg_bank_addr(prg, 0, addr)
        }
        else { addr - ROM_START }
    }

    fn chr_addr(&self, chr: &[u8], addr: usize) -> usize {
        self.chr_bank_addr(chr, self.chr_bank_select, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize;
    }
}
