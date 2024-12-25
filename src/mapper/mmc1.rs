use crate::cart::Mirroring;
use super::{Bank, Mapper, DEFAULT_CHR_BANK_SIZE, DEFAULT_PRG_BANK_SIZE, SRAM_START};

#[derive(Default, PartialEq)]
enum PrgMode { Bank32kb, FixFirst16kb, #[default] FixLast16kb }
#[derive(Default, PartialEq)]
enum ChrMode { #[default] Bank8kb, Bank4kb }

// Mapper 1 https://www.nesdev.org/wiki/MMC1
// Variations SxRom suported

pub struct Mmc1 {
    sram: Vec<u8>,

    submapper: u8,
    shift_reg: u8,
    shift_writes: usize,
    mirroring: Mirroring,
    prg_mode: PrgMode,
    chr_mode: ChrMode,

    chr_bank0_select: Bank,
    chr_bank1_select: Bank,
    last_wrote_chr_bank1: bool,
    prg_bank_select: Bank,
}

impl Default for Mmc1 {
    fn default() -> Self {
        Self { 
            sram: Default::default(), submapper: Default::default(),
            shift_reg: Default::default(), shift_writes: Default::default(), mirroring: Default::default(), prg_mode: Default::default(), chr_mode: Default::default(), chr_bank0_select: Default::default(), chr_bank1_select: Default::default(), last_wrote_chr_bank1: Default::default(), prg_bank_select: Default::default() }
    }
}

impl Mmc1 {
    pub fn new(submapper: u8, sram_size: usize) -> Self {
        let mut res = Self::default();
        res.submapper = submapper;
        res.sram.resize(sram_size, 0);
        res
    }

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
            true  => ChrMode::Bank4kb,
        }
    }

    fn sxrom_register(&self) -> Bank {
        // if self.chr_mode == ChrMode::Bank8kb { return 0; }
        if self.last_wrote_chr_bank1 && self.chr_mode != ChrMode::Bank8kb {
            self.chr_bank1_select
        } else { self.chr_bank0_select }
    }

    fn sram_bank_size(&self) -> usize { 8*1024 }
    fn sram_banks_count(&self) -> usize { self.sram.len() / self.sram_bank_size() }

    fn sram_bank_addr(&self, bank: usize, addr: usize) -> usize {
        let bank_start = (bank % self.sram_banks_count()) * self.sram_bank_size();
        let offset = (addr - SRAM_START) % self.sram_bank_size();
        bank_start + offset
    }

    fn sram_addr(&self, addr: usize) -> usize {
        let bank = self.sram_bank();
        self.sram_bank_addr(bank, addr)
    }

    fn sram_bank(&self) -> Bank {
        let bank_select = self.sxrom_register();

        const KB8: usize  = 8 * 1024;
        const KB16: usize = 16 * 1024;
        const KB32: usize = 32 * 1024;
        match self.sram.len() {
            KB8 => 0,
            KB16 => (bank_select >> 3) & 0b01,
            KB32 => (bank_select >> 2) & 0b11,
            _ => 0,
        }
    }

    fn sram_read(&self, addr: usize) -> u8 {
        if self.sram.is_empty() { return 0; }
        let mapped_addr = self.sram_addr(addr);
        self.sram[mapped_addr]
    }

    fn sram_write(&mut self, addr: usize, val: u8) {
        if self.sram.is_empty() { return; }
        let mapped_addr = self.sram_addr(addr);
        self.sram[mapped_addr] = val;
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

    fn prg_last_bank(&self, prg: &[u8]) -> Bank {
        let banks_count = self.prg_banks_count(prg);

        // 512kb roms acts as if they only have 256kb!
        if prg.len() >= 512 * 1024 { banks_count/2 - 1 }
        else { banks_count - 1 }
    }

    fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {        
        let mut bank = match self.prg_mode {
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

        // SOROM, SUROM and SXROM 512kb prg rom
        if prg.len() >= 512 * 1024 {
            let bank256_select = self.sxrom_register() & 0b1_0000;
            bank += bank256_select;
        }
        
        self.prg_bank_addr(prg, bank, addr)
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
        if (0x6000..=0x7FFF).contains(&addr) {
            self.sram_write(addr, val);
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
                0xA000..=0xBFFF => {
                    self.chr_bank0_select = self.shift_reg as usize & 0b1_1111;
                    self.last_wrote_chr_bank1 = false;
                }
                0xC000..=0xDFFF => {
                    self.chr_bank1_select = self.shift_reg as usize & 0b1_1111;
                    self.last_wrote_chr_bank1 = true;
                }
                0xE000..=0xFFFF => {
                    self.prg_bank_select  = self.shift_reg as usize & 0b1111;
                }
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