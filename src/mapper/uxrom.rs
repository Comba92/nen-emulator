use super::Mapper;

// Mapper 2 https://www.nesdev.org/wiki/UxROM
#[derive(Default)]
pub struct UxRom {
    prg_bank_select: usize,
}
impl Mapper for UxRom {
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        let bank = if (0xC000..=0xFFFF).contains(&addr) {
            self.last_prg_bank(prg)
        } else {
            self.prg_bank_select
        };

        self.read_prg_bank(prg, bank, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = val as usize;
    }
}