use crate::cart::Mirroring;
use super::{Mapper, DEFAULT_CHR_BANK_SIZE, DEFAULT_PRG_BANK_SIZE, SRAM_START};

#[derive(Default)]
enum PrgMode { Bank32kb, FixFirst16kb, #[default] FixLast16kb }
#[derive(Default)]
enum ChrMode { #[default] Bank8kb, Bank4kb }

// Mapper 1 https://www.nesdev.org/wiki/MMC1

pub struct Mmc1 {
    // TODO: Mmc1 can have sram up to 32kb with banking
    // Any game with more than 8 kb of sram will probably crash
    sram: [u8; 8 * 1024],

    shift_reg: u8,
    shift_writes: usize,
    mirroring: Mirroring,
    prg_mode: PrgMode,
    chr_mode: ChrMode,

    chr_bank0_select: usize,
    chr_bank1_select: usize,
    prg_bank_select: usize,
}

impl Default for Mmc1 {
    fn default() -> Self {
        Self { 
            sram: [0; 8 * 1024],
            shift_reg: Default::default(), shift_writes: Default::default(), mirroring: Default::default(), prg_mode: Default::default(), chr_mode: Default::default(), chr_bank0_select: Default::default(), chr_bank1_select: Default::default(), prg_bank_select: Default::default() }
    }
}

impl Mmc1 {
    fn write_ctrl(&mut self, val: u8) {
        self.mirroring = match val & 0b11 {
            0 => Mirroring::SingleScreenA,
            1 => Mirroring::SingleScreenB,
            2 => Mirroring::Vertical,
            3 => Mirroring::Horizontal,
            _ => unreachable!()
        };

        self.prg_mode = match (val >> 2) & 0b11 {
            0 | 1 => PrgMode::Bank32kb,
            2 => PrgMode::FixFirst16kb,
            3 => PrgMode::FixLast16kb,
            _ => unreachable!()
        };

        self.chr_mode = match (val >> 4) != 0 {
            false => ChrMode::Bank8kb,
            true => ChrMode::Bank4kb,
        }
    }
}

impl Mapper for Mmc1 {
    fn prg_bank_size(&self) -> usize {
        match self.prg_mode {
            PrgMode::Bank32kb => DEFAULT_PRG_BANK_SIZE*2,
            _ => DEFAULT_PRG_BANK_SIZE
        }
    }

    fn chr_bank_size(&self) -> usize {
        match self.chr_mode {
            ChrMode::Bank8kb => DEFAULT_CHR_BANK_SIZE,
            ChrMode::Bank4kb => DEFAULT_CHR_BANK_SIZE/2,
        }
    }

    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
        // TODO: 512 kb prg access with last chr bank bit

        let bank = match self.prg_mode {
            PrgMode::Bank32kb => self.prg_bank_select >> 1,
            PrgMode::FixLast16kb => {
                match addr {
                    0x8000..=0xBFFF => self.prg_bank_select,
                    0xC000..=0xFFFF => self.prg_last_bank(prg),
                    _ => unreachable!("Accessed {addr:x}")
                }
            }
            PrgMode::FixFirst16kb => {
                match addr {
                    0x8000..=0xBFFF => 0,
                    0xC000..=0xFFFF => self.prg_bank_select,
                    _ => unreachable!("Accessed {addr:x}")
                }
            }
        };

        self.prg_bank_addr(prg, bank, addr)
    }

    fn prg_read(&mut self, prg: &[u8], addr: usize) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.sram[addr - SRAM_START],
            _ => {
                let mapped_addr = self.prg_addr(prg, addr);
                prg[mapped_addr]
            }
        }
    }

    fn chr_addr(&self, chr: &[u8], addr: usize) -> usize {
        let bank = match self.chr_mode {
            ChrMode::Bank8kb => self.chr_bank0_select >> 1,
            ChrMode::Bank4kb => {
                match addr {
                    0x0000..=0x0FFF => self.chr_bank0_select,
                    0x1000..=0x1FFF => self.chr_bank1_select,
                    _ => unreachable!()
                }
            }
        };

        self.chr_bank_addr(chr, bank, addr)
    }

    fn prg_write(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            self.sram[addr - SRAM_START] = val;
            return;
        }

        if val & 0b1000_0000 != 0 {
            self.shift_reg = 0;
            self.shift_writes = 0;
            self.prg_mode = PrgMode::FixLast16kb;
        } else if self.shift_writes < 5 {
            self.shift_reg = (self.shift_reg >> 1) | ((val & 1) << 4);
            self.shift_writes += 1;
        }
        
        if self.shift_writes >= 5 {
            match addr {
                0x8000..=0x9FFF => self.write_ctrl(self.shift_reg),
                0xA000..=0xBFFF => self.chr_bank0_select = self.shift_reg as usize & 0b1_1111,
                0xC000..=0xDFFF => self.chr_bank1_select = self.shift_reg as usize & 0b1_1111,
                0xE000..=0xFFFF => self.prg_bank_select  = self.shift_reg as usize & 0b1111,
                _ => unreachable!("Accessed {addr:x}")
            }
            
            self.shift_writes = 0;
            self.shift_reg = 0;
        }
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }
}