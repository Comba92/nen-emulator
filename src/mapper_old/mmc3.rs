use crate::cart::Mirroring;

use super::{Bank, Mapper, SRAM_START};

#[derive(Default, serde::Serialize, serde::Deserialize)]
enum PrgMode { #[default] SwapFirst, SwapLast }
#[derive(Default, serde::Serialize, serde::Deserialize)]
enum ChrMode { #[default] BiggerFirst, BiggerLast }
// Mapper 4 https://www.nesdev.org/wiki/MMC3
// Variation MMC6 supported

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Mmc3 {
    bank_select: usize,
    prg_mode: PrgMode,
    chr_mode: ChrMode,
    mirroring: Mirroring,
    
    bank_selects: [Bank; 8],

    sram: Box<[u8]>,
    sram_read_enabled: bool,
    sram_write_enabled: bool,

    irq_counter: u8,
    irq_latch: u8,
    irq_reload: bool,
    irq_enabled: bool,

    irq_requested: Option<()>,
}

impl Default for Mmc3 {
    fn default() -> Self {
        Self { bank_select: Default::default(), prg_mode: Default::default(), chr_mode: Default::default(), mirroring: Default::default(), bank_selects: Default::default(), sram: vec![0; 8 * 1024].into(), sram_read_enabled: Default::default(), sram_write_enabled: Default::default(), irq_counter: Default::default(), irq_latch: Default::default(), irq_reload: Default::default(), irq_enabled: Default::default(), irq_requested: Default::default() }
    }
}

impl Mmc3 {
    fn write_bank_select(&mut self, val: u8) {
        self.bank_select = val as usize & 0b111;

        self.prg_mode = match (val >> 6) & 1 != 0 {
            false => PrgMode::SwapFirst,
            true  => PrgMode::SwapLast,
        };

        self.chr_mode = match (val >> 7) != 0 {
            false => ChrMode::BiggerFirst,
            true  => ChrMode::BiggerLast,
        };
    }

    fn sram_read(&self, addr: usize) -> u8 {
        // The read enabled check might be harmful in an emulator
        // https://www.nesdev.org/wiki/MMC3#iNES_Mapper_004_and_MMC6
        self.sram[addr - SRAM_START]
    }

    fn sram_write(&mut self, addr: usize, val: u8) {
        // The write enabled check might be harmful in an emulator
        self.sram[addr - SRAM_START] = val;
    }
}

#[typetag::serde]
impl Mapper for Mmc3 {
    fn prg_bank_size(&self) -> usize { 8*1024 }
    fn chr_bank_size(&self) -> usize { 1024 }

    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        use PrgMode::*;
        let bank = match (addr, &self.prg_mode) {
            (0x8000..=0x9FFF, SwapFirst) => self.bank_selects[6],
            (0x8000..=0x9FFF, SwapLast)  => self.prg_last_bank(prg)-1,
            (0xA000..=0xBFFF, _) => self.bank_selects[7],
            (0xC000..=0xDFFF, SwapFirst) => self.prg_last_bank(prg)-1,
            (0xC000..=0xDFFF, SwapLast) => self.bank_selects[6],
            (0xE000..=0xFFFF, _) => self.prg_last_bank(prg),
            _ => unreachable!()
        };

        self.prg_bank_addr(prg, bank, addr)
    }

    fn chr_addr(&self, chr: &[u8], addr: usize) -> usize {
        use ChrMode::*;
        let bank = match(addr, &self.chr_mode) {
            (0x0000..=0x03FF, BiggerFirst) => self.bank_selects[0],
            (0x0400..=0x07FF, BiggerFirst) => self.bank_selects[0]+1,
            (0x0800..=0x0BFF, BiggerFirst) => self.bank_selects[1],
            (0x0C00..=0x0FFF, BiggerFirst) => self.bank_selects[1]+1,
            (0x1000..=0x13FF, BiggerFirst) => self.bank_selects[2],
            (0x1400..=0x17FF, BiggerFirst) => self.bank_selects[3],
            (0x1800..=0x1BFF, BiggerFirst) => self.bank_selects[4],
            (0x1C00..=0x1FFF, BiggerFirst) => self.bank_selects[5],

            (0x0000..=0x03FF, BiggerLast) => self.bank_selects[2],
            (0x0400..=0x07FF, BiggerLast) => self.bank_selects[3],
            (0x0800..=0x0BFF, BiggerLast) => self.bank_selects[4],
            (0x0C00..=0x0FFF, BiggerLast) => self.bank_selects[5],
            (0x1000..=0x13FF, BiggerLast) => self.bank_selects[0],
            (0x1400..=0x17FF, BiggerLast) => self.bank_selects[0]+1,
            (0x1800..=0x1BFF, BiggerLast) => self.bank_selects[1],
            (0x1C00..=0x1FFF, BiggerLast) => self.bank_selects[1]+1,

            _ => unreachable!()
        };

        self.chr_bank_addr(chr, bank, addr)
    }

    fn prg_read(&mut self, prg: &[u8], addr: usize) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.sram_read(addr),
            _ => {
                let mapped_addr = self.prg_addr(prg, addr);
                prg[mapped_addr]
            }
        }
    }

    fn prg_write(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        let addr_even = addr % 2 == 0;
        match (addr, addr_even) {
            (0x6000..=0x7FFF, _)    => self.sram_write(addr, val),
            (0x8000..=0x9FFE, true) => self.write_bank_select(val),
            (0x8001..=0x9FFF, false) => match self.bank_select {
                0 | 1 => self.bank_selects[self.bank_select] = val as usize & !1,
                _ => self.bank_selects[self.bank_select] = val as usize,
            }
            (0xA000..=0xBFFE, true) => match val & 1 != 0 {
                false => self.mirroring = Mirroring::Vertical,
                true  => self.mirroring = Mirroring::Horizontal,
            }
            (0xA001..=0xBFFF, false) => {
                self.sram_write_enabled = val & 0b0100_0000 == 0;
                self.sram_read_enabled  = val & 0b1000_0000 != 0;
            }
            (0xC000..=0xDFFE, true) => self.irq_latch = val,
            (0xC001..=0xDFFF, false) => self.irq_reload = true,
            (0xE000..=0xFFFE, true) => {
                self.irq_enabled = false;
                self.irq_requested = None;
            }
            (0xE001..=0xFFFF, false) => self.irq_enabled = true,
            _ => unreachable!()
        }
    }

    fn notify_scanline(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_enabled && self.irq_counter == 0 {
            self.irq_requested = Some(());
        }
    }

    fn poll_irq(&mut self) -> bool {
        self.irq_requested.is_some()
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }
}