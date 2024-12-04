use core::cell::RefCell;
use std::rc::Rc;

use crate::cart::Mirroring;

pub type CartMapper = Rc<RefCell<dyn Mapper>>;
pub trait Mapper {
    // Default NRom PRG banking
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        // if it only has 16KiB, then mirror to first bank
        if prg.len() == self.prg_bank_size() { 
            self.read_prg_bank(prg, 0, addr)
        }
        else { prg[addr - ROM_START] }
    }
    
    // Default NRom CHR banking
    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 { chr[addr] }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}
    fn write_chr(&mut self, chr: &mut[u8], addr: usize, val: u8) { chr[addr] = val; }

    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE }
    fn chr_bank_size(&self) -> usize { DEFAULT_CHR_BANK_SIZE }

    fn prg_banks_count(&self, prg: &[u8]) -> usize { prg.len() / self.prg_bank_size() }
    fn chr_banks_count(&self, chr: &[u8]) -> usize { chr.len() / self.chr_bank_size() }
    
    fn last_prg_bank(&self, prg: &[u8]) -> usize { self.prg_banks_count(prg) - 1 }

    fn read_prg_bank(&self, prg: &[u8], bank: usize, addr: usize) -> u8 {
        let bank_start = (bank % self.prg_banks_count(prg)) * self.prg_bank_size();
        let offset = (addr - ROM_START) % self.prg_bank_size();
        prg[bank_start + offset]
    }

    fn read_chr_bank(&self, chr: &[u8], bank: usize, addr: usize) -> u8 {
        let bank_start = (bank % self.chr_banks_count(chr)) * self.chr_bank_size();
        let offset = addr % self.chr_bank_size();
        chr[bank_start + offset]
    }

    fn mirroring(&self) -> Option<Mirroring> { None }
    fn scanline_ended(&mut self) {}
    fn poll_irq(&mut self) -> bool { false }
}

pub fn new_mapper_from_id(id: u8) -> Result<CartMapper, String> {
    let mapper: CartMapper = match id {
        0  => Rc::new(RefCell::new(NRom)),
        1  => Rc::new(RefCell::new(Mmc1::default())),
        2  => Rc::new(RefCell::new(UxRom::default())),
        3  => Rc::new(RefCell::new(INesMapper003::default())),
        4  => Rc::new(RefCell::new(Mmc3::default())),
        // 5 => // TODO
        7  => Rc::new(RefCell::new(AxRom::default())),
        9  => Rc::new(RefCell::new(Mmc2::default())),
        11 => Rc::new(RefCell::new(ColorDreams::default())),
        // 64 => // TODO

        66 => Rc::new(RefCell::new(GxRom::default())),
        _ => return Err(format!("Mapper {id} not implemented"))
    };

    Ok(mapper)
}

const ROM_START: usize = 0x8000;
const DEFAULT_PRG_BANK_SIZE: usize = 16*1024; // 16 KiB
const DEFAULT_CHR_BANK_SIZE: usize = 8*1024; // 8 KiB

pub struct Dummy;
impl Mapper for Dummy {
    fn read_prg(&mut self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn read_chr(&mut self, _chr: &[u8], _addr: usize) -> u8 { 0 }
}

// Mapper 0 https://www.nesdev.org/wiki/NROM
pub struct NRom;
impl Mapper for NRom {}


#[derive(Default, Debug)]
enum Mmc1PrgMode { Bank32kb, FixFirst16kb, #[default] FixLast16kb }
#[derive(Default, Debug)]
enum Mmc1ChrMode { #[default] Bank8kb, Bank4kb }

// Mapper 1 https://www.nesdev.org/wiki/MMC1
#[derive(Default, Debug)]
pub struct Mmc1 {
    shift_reg: u8,
    shift_writes: usize,
    mirroring: Mirroring,
    prg_mode: Mmc1PrgMode,
    chr_mode: Mmc1ChrMode,

    chr_bank0_select: usize,
    chr_bank1_select: usize,
    prg_bank_select: usize,
}

impl Mmc1 {
    fn write_ctrl(&mut self, val: u8) {
        self.mirroring = match val & 0b11 {
            0 => Mirroring::SingleScreenFirstPage,
            1 => Mirroring::SingleScreenSecondPage,
            2 => Mirroring::Vertically,
            3 => Mirroring::Horizontally,
            _ => unreachable!()
        };

        self.prg_mode = match (val >> 2) & 0b11 {
            0 | 1 => Mmc1PrgMode::Bank32kb,
            2 => Mmc1PrgMode::FixFirst16kb,
            3 => Mmc1PrgMode::FixLast16kb,
            _ => unreachable!()
        };

        self.chr_mode = match (val >> 4) != 0 {
            false => Mmc1ChrMode::Bank8kb,
            true => Mmc1ChrMode::Bank4kb,
        }
    }
}

impl Mapper for Mmc1 {
    fn prg_bank_size(&self) -> usize {
        match self.prg_mode {
            Mmc1PrgMode::Bank32kb => DEFAULT_PRG_BANK_SIZE*2,
            _ => DEFAULT_PRG_BANK_SIZE
        }
    }

    fn chr_bank_size(&self) -> usize {
        match self.chr_mode {
            Mmc1ChrMode::Bank8kb => DEFAULT_CHR_BANK_SIZE,
            Mmc1ChrMode::Bank4kb => DEFAULT_CHR_BANK_SIZE/2,
        }
    }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        let bank = match self.prg_mode {
            Mmc1PrgMode::Bank32kb => self.prg_bank_select >> 1,
            Mmc1PrgMode::FixLast16kb => {
                match addr {
                    0x8000..=0xBFFF => self.prg_bank_select,
                    0xC000..=0xFFFF => self.last_prg_bank(prg),
                    _ => unreachable!()
                }
            }
            Mmc1PrgMode::FixFirst16kb => {
                match addr {
                    0x8000..=0xBFFF => 0,
                    0xC000..=0xFFFF => self.prg_bank_select,
                    _ => unreachable!()
                }
            }
        };

        self.read_prg_bank(prg, bank, addr)
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        let bank = match self.chr_mode {
            Mmc1ChrMode::Bank8kb => self.chr_bank0_select >> 1,
            Mmc1ChrMode::Bank4kb => {
                match addr {
                    0x0000..=0x0FFF => self.chr_bank0_select,
                    0x1000..=0x1FFF => self.chr_bank1_select,
                    _ => unreachable!()
                }
            }
        };

        self.read_chr_bank(chr, bank, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        if val & 0b1000_0000 != 0 {
            self.shift_reg = 0;
            self.shift_writes = 0;
            self.prg_mode = Mmc1PrgMode::FixLast16kb;
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
                _ => unreachable!()
            }
            
            self.shift_writes = 0;
            self.shift_reg = 0;
        }
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }
}


#[derive(Default)]
pub enum Mmc3PrgMode { #[default] SwapFirst, SwapLast }
#[derive(Default)]
pub enum Mmc3ChrMode { #[default] BiggerFirst, BiggerLast }
// Mapper 4 https://www.nesdev.org/wiki/MMC3
#[derive(Default)]
pub struct Mmc3 {
    bank_select: usize,
    prg_mode: Mmc3PrgMode,
    chr_mode: Mmc3ChrMode,
    mirroring: Mirroring,
    
    bank_regs: [usize; 8],

    prg_ram_read_enabled: bool,
    prg_ram_write_enabled: bool,

    irq_counter: u8,
    irq_latch: u8,
    irq_reload: bool,
    irq_on: bool,

    irq_requested: Option<()>,
}
impl Mmc3 {
    fn write_bank_select(&mut self, val: u8) {
        self.bank_select = val as usize & 0b111;

        self.prg_mode = match (val >> 6) & 1 != 0 {
            false => Mmc3PrgMode::SwapFirst,
            true  => Mmc3PrgMode::SwapLast,
        };

        self.chr_mode = match (val >> 7) != 0 {
            false => Mmc3ChrMode::BiggerFirst,
            true  => Mmc3ChrMode::BiggerLast,
        };
    }
}

impl Mapper for Mmc3 {
    fn prg_bank_size(&self) -> usize { 8*1024 }
    fn chr_bank_size(&self) -> usize { 1024 }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        use Mmc3PrgMode::*;
        let bank = match (addr, &self.prg_mode) {
            (0x8000..=0x9FFF, SwapFirst) => self.bank_regs[6],
            (0x8000..=0x9FFF, SwapLast)  => self.last_prg_bank(prg)-1,
            (0xA000..=0xBFFF, _) => self.bank_regs[7],
            (0xC000..=0xDFFF, SwapFirst) => self.last_prg_bank(prg)-1,
            (0xC000..=0xDFFF, SwapLast) => self.bank_regs[6],
            (0xE000..=0xFFFF, _) => self.last_prg_bank(prg),
            _ => unreachable!()
        };

        self.read_prg_bank(prg, bank, addr)
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        use Mmc3ChrMode::*;
        let bank = match(addr, &self.chr_mode) {
            (0x0000..=0x03FF, BiggerFirst) => self.bank_regs[0],
            (0x0400..=0x07FF, BiggerFirst) => self.bank_regs[0]+1,
            (0x0800..=0x0BFF, BiggerFirst) => self.bank_regs[1],
            (0x0C00..=0x0FFF, BiggerFirst) => self.bank_regs[1]+1,
            (0x1000..=0x13FF, BiggerFirst) => self.bank_regs[2],
            (0x1400..=0x17FF, BiggerFirst) => self.bank_regs[3],
            (0x1800..=0x1BFF, BiggerFirst) => self.bank_regs[4],
            (0x1C00..=0x1FFF, BiggerFirst) => self.bank_regs[5],

            (0x0000..=0x03FF, BiggerLast) => self.bank_regs[2],
            (0x0400..=0x07FF, BiggerLast) => self.bank_regs[3],
            (0x0800..=0x0BFF, BiggerLast) => self.bank_regs[4],
            (0x0C00..=0x0FFF, BiggerLast) => self.bank_regs[5],
            (0x1000..=0x13FF, BiggerLast) => self.bank_regs[0],
            (0x1400..=0x17FF, BiggerLast) => self.bank_regs[0]+1,
            (0x1800..=0x1BFF, BiggerLast) => self.bank_regs[1],
            (0x1C00..=0x1FFF, BiggerLast) => self.bank_regs[1]+1,

            _ => unreachable!()
        };

        self.read_chr_bank(chr, bank, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        let addr_even = addr % 2 == 0;
        match (addr, addr_even) {
            (0x8000..=0x9FFE, true) => self.write_bank_select(val),
            (0x8001..=0x9FFF, false) => match self.bank_select {
                0 | 1 => self.bank_regs[self.bank_select] = val as usize & !1,
                _ => self.bank_regs[self.bank_select] = val as usize,
            }
            (0xA000..=0xBFFE, true) => match val & 1 != 0 {
                false => self.mirroring = Mirroring::Vertically,
                true  => self.mirroring = Mirroring::Horizontally,
            }
            (0xA001..=0xBFFF, false) => {
                self.prg_ram_write_enabled = val & 0b0100_0000 == 0;
                self.prg_ram_read_enabled  = val & 0b1000_0000 != 0;
            }
            (0xC000..=0xDFFE, true) => self.irq_latch = val,
            (0xC001..=0xDFFF, false) => self.irq_reload = true,
            (0xE000..=0xFFFE, true) => {
                self.irq_on = false;
                self.irq_requested = None;
                // acknowledge any pending interrupts
            }
            (0xE001..=0xFFFF, false) => self.irq_on = true,
            _ => unreachable!()
        }
    }

    fn scanline_ended(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_on && self.irq_counter == 0 {
            self.irq_requested = Some(());
        }
    }

    fn poll_irq(&mut self) -> bool {
        self.irq_requested.take().is_some()
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }
}

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

// Mapper 3 https://www.nesdev.org/wiki/INES_Mapper_003
#[derive(Default)]
pub struct INesMapper003 {
    chr_bank_select: usize,
}
impl Mapper for INesMapper003 {
    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        self.read_chr_bank(chr, self.chr_bank_select, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize;
    }
}


// Mapper 7 https://www.nesdev.org/wiki/AxROM
#[derive(Default)]
pub struct AxRom {
    pub prg_bank_select: usize,
    pub mirroring_page: Mirroring,
}

impl Mapper for AxRom {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE*2 }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        self.read_prg_bank(prg, self.prg_bank_select, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_0111) as usize;
        
        self.mirroring_page = match val & 0b0001_0000 != 0 {
            false   => Mirroring::SingleScreenFirstPage,
            true    => Mirroring::SingleScreenSecondPage,
        };
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring_page)
    }
}

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

#[derive(Default, Clone, Copy)]
pub enum Mmc2Latch { FD, #[default] FE }

// Mapper 9 https://www.nesdev.org/wiki/MMC2
#[derive(Default)]
pub struct Mmc2 {
    pub prg_bank_select: usize,
    pub chr_bank0_select: [usize; 2],
    pub chr_bank1_select: [usize; 2],
    pub latch: [Mmc2Latch; 2],
    pub mirroring: Mirroring,
}
impl Mapper for Mmc2 {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE/2 }
    fn chr_bank_size(&self) -> usize { DEFAULT_CHR_BANK_SIZE/2 }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        let bank = match addr {
            0x8000..=0x9FFF => self.prg_bank_select,
            0xA000..=0xBFFF => self.last_prg_bank(prg)-2,
            0xC000..=0xDFFF => self.last_prg_bank(prg)-1,
            0xE000..=0xFFFF => self.last_prg_bank(prg),
            _ => unreachable!()
        };

        self.read_prg_bank(prg, bank, addr)
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        let bank = match addr {
            0x0000..=0x0FFF => self.chr_bank0_select[self.latch[0] as usize],
            0x1000..=0x1FFF => self.chr_bank1_select[self.latch[1] as usize],
            _ => unreachable!()
        };
        
        match addr {
            0x0FD8 => self.latch[0] = Mmc2Latch::FD,
            0x0FE8 => self.latch[0] = Mmc2Latch::FE,
            0x1FD8..=0x1FDF => self.latch[1] = Mmc2Latch::FD,
            0x1FE8..=0x1FEF => self.latch[1] = Mmc2Latch::FE,
            _ => {}
        }

        self.read_chr_bank(chr, bank, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        let val = val as usize & 0b1_1111;

        match addr {
            0xA000..=0xAFFF => self.prg_bank_select = val & 0b1111,
            0xB000..=0xBFFF => {
                self.chr_bank0_select[0] = val;
            }
            0xC000..=0xCFFF => {
                self.chr_bank0_select[1] = val;
            }
            0xD000..=0xDFFF => {
                self.chr_bank1_select[0] = val;
            }
            0xE000..=0xEFFF => {
                self.chr_bank1_select[1] = val;
            }
            0xF000..=0xFFFF => {
                self.mirroring = match val & 1 {
                    0 => Mirroring::Vertically,
                    1 => Mirroring::Horizontally,
                    _ => unreachable!()
                };
            }
            _ => unreachable!()
        }
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }
}


// Mapper 11 https://www.nesdev.org/wiki/Color_Dreams
#[derive(Default)]
pub struct ColorDreams {
    pub prg_bank_select: usize,
    pub chr_bank_select: usize,
}
impl Mapper for ColorDreams {
    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE*2 }

    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        self.read_prg_bank(prg, self.prg_bank_select, addr)
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        self.read_chr_bank(chr, self.chr_bank_select, addr)
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_0011) as usize;
        self.chr_bank_select = ((val & 0b1111_0000) >> 4) as usize;
    }
}