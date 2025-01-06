use super::Mapper;

// Mapper 2 https://www.nesdev.org/wiki/UxROM
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct UxRom {
    prg_bank_select: usize,
}

#[typetag::serde]
impl Mapper for UxRom {
    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        let bank = match addr {
            0xC000..=0xFFFF => self.prg_last_bank(prg),
            // TODO: we're not handling access to 0x6000..=0x7FFF
            _ => self.prg_bank_select,
        };

        self.prg_bank_addr(prg, bank, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = val as usize;
    }
}