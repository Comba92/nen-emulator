use super::Mapper;

// Mapper 3 https://www.nesdev.org/wiki/INES_Mapper_003
#[derive(Default)]
pub struct INesMapper003 {
    chr_bank_select: usize,
}
impl Mapper for INesMapper003 {
    fn chr_addr(&mut self, chr: &[u8], addr: usize) -> usize {
        self.chr_bank_addr(chr, self.chr_bank_select, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize;
    }
}
