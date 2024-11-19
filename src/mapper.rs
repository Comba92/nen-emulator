use core::cell::RefCell;
use std::rc::Rc;

use bitflags::bitflags;

use crate::cart::Mirroring;

pub type CartMapper = Rc<RefCell<dyn Mapper>>;
pub trait Mapper {
    // Default NRom PRG banking
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        // if it only has 16KiB, then mirror first bank
        if prg.len() == PRG_BANK_SIZE { prg[addr % (PRG_BANK_SIZE)] }
        else { prg[addr] }
    }
    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}

    // Default NRom CHR banking
    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        chr[addr]
    }
    fn write_chr(&mut self, chr: &mut[u8], addr: usize, val: u8) {
        chr[addr] = val;
    }

    fn mirroring(&self) -> Option<Mirroring> { None }
    fn poll_irq(&mut self) -> bool { false }
}

pub fn new_mapper_from_id(id: u8) -> Result<CartMapper, String> {
    let mapper: CartMapper = match id {
        0 => Rc::new(RefCell::new(NRom)),
        1 => Rc::new(RefCell::new(Mmc1::default())),
        2 => Rc::new(RefCell::new(UxRom::default())),
        3 => Rc::new(RefCell::new(INesMapper003::default())),
        4 => Rc::new(RefCell::new(Mmc3::default())),
        7 => Rc::new(RefCell::new(AxRom::default())),
        9 => Rc::new(RefCell::new(Mmc2::default())),
        11 => Rc::new(RefCell::new(ColorDreams::default())),
        66 => Rc::new(RefCell::new(GxRom::default())),
        _ => return Err(format!("Mapper {id} not implemented"))
    };

    Ok(mapper)
}

const PRG_BANK_SIZE: usize = 16*1024; // 16 KiB
const CHR_BANK_SIZE: usize = 8*1024; // 8 KiB

pub struct Dummy;
impl Mapper for Dummy {
    fn read_prg(&mut self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn read_chr(&mut self, _chr: &[u8], _addr: usize) -> u8 { 0 }
}

// Mapper 0 https://www.nesdev.org/wiki/NROM
pub struct NRom;
impl Mapper for NRom {}

bitflags! {
    #[derive(Debug, Default, Clone)]
    pub struct Mmc1Ctrl: u8 {
        const nametbl_mirror = 0b00011;
        const prg_bank_mode  = 0b01100;
        const chr_bank_mode  = 0b10000;
    }
}
impl Mmc1Ctrl {
    pub fn mirroring(&self) -> Mirroring {
        match self.clone().intersection(Mmc1Ctrl::nametbl_mirror).bits() {
            0 => Mirroring::SingleScreenFirstPage,
            1 => Mirroring::SingleScreenSecondPage,
            2 => Mirroring::Vertically,
            3 => Mirroring::Horizontally,
            _ => unreachable!()
        }
    }

    pub fn prg_mode(&self) -> Mmc1PrgMode {
        match self.clone().intersection(Mmc1Ctrl::prg_bank_mode).bits() >> 2 {
            0 | 1 => Mmc1PrgMode::Bank32kb,
            2 => Mmc1PrgMode::BankLast16kb,
            3 => Mmc1PrgMode::BankFirst16kb,
            _ => unreachable!()
        }
    }

    pub fn chr_mode(&self) -> Mmc1ChrMode {
        match self.contains(Mmc1Ctrl::chr_bank_mode) {
            false => Mmc1ChrMode::Bank8kb,
            true => Mmc1ChrMode::Bank4kb,
        }
    }
}

pub enum Mmc1PrgMode { Bank32kb, BankFirst16kb, BankLast16kb }
pub enum Mmc1ChrMode { Bank8kb, Bank4kb }

// Mapper 1 https://www.nesdev.org/wiki/MMC1
#[derive(Debug, Default)]
pub struct Mmc1 {
    shift_reg: u8,
    shift_writes: usize,
    ctrl: Mmc1Ctrl,

    chr_bank0_select: usize,
    chr_bank1_select: usize,
    prg_bank_select: usize,
}

impl Mapper for Mmc1 {
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        match self.ctrl.prg_mode() {
            Mmc1PrgMode::Bank32kb => {
                let bank_start = (self.prg_bank_select >> 1) * PRG_BANK_SIZE*2;
                prg[bank_start + (addr % (PRG_BANK_SIZE*2))]
            }
            Mmc1PrgMode::BankLast16kb => {
                let bank_select = match addr {
                    0x0000..=0x3FFF => 0,
                    0x4000..=0x7FFF => self.prg_bank_select * PRG_BANK_SIZE,
                    _ => unreachable!()
                };
                prg[bank_select + (addr % PRG_BANK_SIZE)]
            }
            Mmc1PrgMode::BankFirst16kb => {
                let bank_select = match addr {
                    0x0000..=0x3FFF => self.prg_bank_select * PRG_BANK_SIZE,
                    0x4000..=0x7FFF => prg.len() - PRG_BANK_SIZE,
                    _ => unreachable!()
                };
                prg[bank_select + (addr % PRG_BANK_SIZE)]
            }
        }
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        match self.ctrl.chr_mode() {
            Mmc1ChrMode::Bank8kb => {
                let bank_start = (self.chr_bank0_select >> 1) * CHR_BANK_SIZE;
                chr[bank_start + (addr % CHR_BANK_SIZE)]
            }
            Mmc1ChrMode::Bank4kb => {
                let bank_select = match addr {
                    0x0000..=0x0FFF => self.chr_bank0_select,
                    0x1000..=0x1FFF => self.chr_bank1_select,
                    _ => unreachable!()
                };

                chr[bank_select * (CHR_BANK_SIZE/2) + (addr % (CHR_BANK_SIZE/2))]
            }
        }
    }

    fn write_prg(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        if val & 0b1000_0000 != 0 {
            self.shift_reg = 0;
            self.shift_writes = 0;
            self.ctrl = self.ctrl.clone().union(Mmc1Ctrl::from_bits_retain(0x0C));
        } else if self.shift_writes < 5 {
            self.shift_reg = (self.shift_reg >> 1) | ((val & 1) << 4);
            self.shift_writes += 1;
        }
        
        if self.shift_writes >= 5 {
            self.shift_writes = 0;

            match addr {
                0..=0x1FFF => self.ctrl = Mmc1Ctrl::from_bits_retain(self.shift_reg),
                0x2000..=0x3FFF => self.chr_bank0_select = self.shift_reg as usize,
                0x4000..=0x5FFF => self.chr_bank1_select = self.shift_reg as usize,
                0x6000..=0x7FFF => self.prg_bank_select = self.shift_reg as usize,
                _ => unreachable!()
            }

            self.shift_reg = 0;
        }
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.ctrl.mirroring())
    }
}


bitflags! {
    #[derive(Default, Clone)]
    pub struct Mmc3Select: u8 {
        const bank_update   = 0b0000_0111;
        const prg_bank_mode = 0b0100_0000;
        const chr_bank_mode = 0b1000_0000;
    }
}
impl Mmc3Select {
    pub fn prg_mode(&self) -> Mmc3PrgMode {
        match self.contains(Mmc3Select::prg_bank_mode) {
            false => Mmc3PrgMode::BankFirst,
            true  => Mmc3PrgMode::BankLast,
        }
    }

    pub fn chr_mode(&self) -> Mmc3ChrMode {
        match self.contains(Mmc3Select::chr_bank_mode) {
            false => Mmc3ChrMode::BiggerFirst,
            true  => Mmc3ChrMode::BiggerLast,
        }
    }
}

#[derive(Default)]
pub enum Mmc3PrgMode { #[default] BankFirst, BankLast }
#[derive(Default)]
pub enum Mmc3ChrMode { #[default] BiggerFirst, BiggerLast }
// Mapper 4 https://www.nesdev.org/wiki/MMC3
pub struct Mmc3 {
    pub bank_select: Mmc3Select,
    pub mirroring: Mirroring,

    pub bank_regs: [usize; 8],

    pub prg_ram_read_on: bool,
    pub prg_ram_write_on: bool,

    pub irq_counter: u8,
    pub irq_latch: u8,
    pub irq_on: bool,
}
impl Default for Mmc3 {
    fn default() -> Self {
        Self { bank_select: Mmc3Select::prg_bank_mode, mirroring: Default::default(), bank_regs: Default::default(), prg_ram_read_on: Default::default(), prg_ram_write_on: Default::default(), irq_counter: Default::default(), irq_latch: Default::default(), irq_on: Default::default() }
    }
} 
impl Mapper for Mmc3 {
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        use Mmc3PrgMode::*;
        let bank_start = match (addr, self.bank_select.prg_mode()) {
            (0x0000..=0x1FFF, BankFirst) => {
                self.bank_regs[6] * PRG_BANK_SIZE
            }
            (0x0000..=0x1FFF, BankLast) => {
                prg.len() - PRG_BANK_SIZE*2
            }
            (0x2000..=0x3FFF, _) => {
                self.bank_regs[7] * PRG_BANK_SIZE
            }
            (0x4000..=0x5FFF, BankFirst) => {
                prg.len() - PRG_BANK_SIZE*2
            }
            (0x4000..=0x5FFF, BankLast) => {
                self.bank_regs[6] * PRG_BANK_SIZE
            }
            (0x6000..=0x7FFF, _) => {
                prg.len() - PRG_BANK_SIZE
            }
            _ => unreachable!()
        };

        prg[bank_start + (addr % CHR_BANK_SIZE)]
    }
    
    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        use Mmc3ChrMode::*;
        let bank_start = match (addr, self.bank_select.chr_mode()) {
            (0x0000..=0x03FF, BiggerFirst) => {
                self.bank_regs[0]
            }
            (0x0400..=0x07FF, BiggerFirst) => {
                self.bank_regs[0] + 1
            }
            (0x0800..=0x0BFF, BiggerFirst) => {
                self.bank_regs[1]
            }
            (0x0C00..=0x0FFF, BiggerFirst) => {
                self.bank_regs[1] + 1
            }
            (0x1000..=0x13FF, BiggerFirst) => {
                self.bank_regs[2]
            }
            (0x1400..=0x17FF, BiggerFirst) => {
                self.bank_regs[3]
            }
            (0x1800..=0x1BFF, BiggerFirst) => {
                self.bank_regs[4]
            }
            (0x1C00..=0x1FFF, BiggerFirst) => {
                self.bank_regs[5]
            }

            (0x0000..=0x03FF, BiggerLast) => {
                self.bank_regs[2] 
            }
            (0x0400..=0x07FF, BiggerLast) => {
                self.bank_regs[3] 
            }
            (0x0800..=0x0BFF, BiggerLast) => {
                self.bank_regs[4]
            }
            (0x0C00..=0x0FFF, BiggerLast) => {
                self.bank_regs[5]
            }
            (0x1000..=0x13FF, BiggerLast) => {
                self.bank_regs[0]
            }
            (0x1400..=0x17FF, BiggerLast) => {
                self.bank_regs[0] + 1
            }
            (0x1800..=0x1BFF, BiggerLast) => {
                self.bank_regs[1]
            }
            (0x1C00..=0x1FFF, BiggerLast) => {
                self.bank_regs[1] + 1
            }
            _ => {unreachable!()}
        };

        chr[bank_start * CHR_BANK_SIZE + (addr % CHR_BANK_SIZE)]
    }

    fn write_prg(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        let addr_even = addr % 2 == 0;
        match (addr, addr_even) {
            (0x0000..=0x1FFE, true) => {
                self.bank_select = Mmc3Select::from_bits_retain(val);
            }
            (0x0001..=0x1FFF, false) => {
                let reg = self.bank_select.clone()
                    .intersection(Mmc3Select::bank_update)
                    .bits();
                self.bank_regs[reg as usize] = val as usize;
            }
            (0x2000..=0x3FFE, true) => {
                self.mirroring = match val & 1 != 0{
                    true  => Mirroring::Horizontally,
                    false => Mirroring::Vertically
                };
            }
            (0x2001..=0x3FFF, false) => {
                self.prg_ram_write_on = val & 0b0100_0000 == 0;
                self.prg_ram_read_on  = val & 0b1000_0000 != 0;
            }
            (0x4000..=0x5FFE, true) => {
                self.irq_latch = val;
            }
            (0x4001..=0x5FFF, false) => {
                self.irq_counter = 0;
            }
            (0x6000..=0x7FFE, true) => {
                self.irq_on = false;
                // acknowledge any pending interrupts
            }
            (0x6001..=0x7FFF, false) => {
                self.irq_on = true;
            }
            _ => unreachable!()
        }
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
        if (0x4000..=0x7FFF).contains(&addr) {
            let last_bank_start = prg.len() - PRG_BANK_SIZE;
            prg[last_bank_start + (addr % PRG_BANK_SIZE)]
        } else {
            prg[self.prg_bank_select + (addr % PRG_BANK_SIZE)]
        }
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_1111) as usize * PRG_BANK_SIZE;
    }
}

// Mapper 3 https://www.nesdev.org/wiki/INES_Mapper_003
#[derive(Default)]
pub struct INesMapper003 {
    chr_bank_select: usize,
}
impl Mapper for INesMapper003 {
    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + (addr % CHR_BANK_SIZE)]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize * CHR_BANK_SIZE;
    }
}


// Mapper 7 https://www.nesdev.org/wiki/AxROM
#[derive(Default)]
pub struct AxRom {
    pub prg_bank_select: usize,
    pub mirroring_page: Mirroring,
}

impl Mapper for AxRom {
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + (addr % (PRG_BANK_SIZE*2))]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        // Banks are 32kb big, not 16kB!
        self.prg_bank_select = (val & 0b0000_0111) as usize * PRG_BANK_SIZE*2;
        
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
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + (addr % (PRG_BANK_SIZE*2))]
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + (addr % CHR_BANK_SIZE)]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize * CHR_BANK_SIZE;
        // Banks are 32kb big, not 16kB!
        self.prg_bank_select = ((val & 0b0011_0000) >> 4) as usize * PRG_BANK_SIZE*2;
    }
}

#[derive(Default, Clone, Copy)]
pub enum Mmc2Latch { FD, #[default] FE }
// Mapper 9 https://www.nesdev.org/wiki/MMC2
// TODO: not working
#[derive(Default)]
pub struct Mmc2 {
    pub prg_bank_select: usize,
    pub chr_bank0_select: usize,
    pub chr_bank1_select: [usize; 2],
    pub latch: Mmc2Latch,
    pub mirroring: Mirroring,
}
impl Mapper for Mmc2 {
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        // last three 8kb prg banks
        if (0x2000..=0x7FFF).contains(&addr) {
            let last_three_banks = prg.len() - (CHR_BANK_SIZE*3);
            prg[last_three_banks + (addr % (CHR_BANK_SIZE*3))]
        } else {
            prg[self.prg_bank_select + (addr % CHR_BANK_SIZE)]
        }
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        // last two switchable chr banks
        let mapped = if (0x1000..=0x1FFF).contains(&addr) {
            self.chr_bank1_select[self.latch as usize]
        } else {
            self.chr_bank0_select
        };

        // match addr {
        //     0x0FD8 => self.latch = Mmc2Latch::FD,
        //     0x0FE8 => self.latch = Mmc2Latch::FE,
        //     0x1FD8..=0x1FDF => self.latch = Mmc2Latch::FD,
        //     0x1FE8..=0x1FEF => self.latch = Mmc2Latch::FE,
        //     _ => {}
        // }

        match addr {
            (0xFD0..=0xFDF) | (0x1FD0..=0x1FDF) => self.latch = Mmc2Latch::FD,
            (0xFE0..=0xFEF) | (0x1FE0..=0x1FEF) => self.latch = Mmc2Latch::FE,
            _ => {}
        }

        chr[mapped * (CHR_BANK_SIZE/2) + (addr % (CHR_BANK_SIZE/2))]
    }

    fn write_prg(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
        let val = val as usize & 0b1_1111;

        match addr {
            0x2000..=0x2FFF => self.prg_bank_select = (val & 0b1111) * CHR_BANK_SIZE,
            // 0x3000..=0x3FFF => {
            //     // set 0xFD/0 bank select
            //     self.chr_bank0_select[0] = val * (CHR_BANK_SIZE/2);
            // }
            // 0x4000..=0x4FFF => {
            //     // set 0xFE/0 bank select
            //     self.chr_bank0_select[1] = val * (CHR_BANK_SIZE/2);
            // }
            0x3000..=0x4FFF => {
                self.chr_bank0_select = val;
            }
            0x5000..=0x5FFF => {
                self.chr_bank1_select[0] = val;
            }
            0x6000..=0x6FFF => {
                self.chr_bank1_select[1] = val;
            }
            0x7000..=0x7FFF => {
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
    fn read_prg(&mut self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + (addr % (PRG_BANK_SIZE*2))]
    }

    fn read_chr(&mut self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + (addr % CHR_BANK_SIZE)]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        // Banks are 32kb big, not 16kB!
        self.prg_bank_select = (val & 0b0000_0011) as usize * PRG_BANK_SIZE*2;
        self.chr_bank_select = ((val & 0b1111_0000) >> 4) as usize * CHR_BANK_SIZE;
    }
}