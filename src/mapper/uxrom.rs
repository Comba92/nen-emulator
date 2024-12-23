use super::Mapper;

// Mapper 2 https://www.nesdev.org/wiki/UxROM
#[derive(Default)]
pub struct UxRom {
    prg_bank_select: usize,
}
impl Mapper for UxRom {
    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        let bank = if (0xC000..=0xFFFF).contains(&addr) {
            self.prg_last_bank(prg)
        } else {
            self.prg_bank_select
        };

        self.prg_bank_addr(prg, bank, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = val as usize;
    }
}