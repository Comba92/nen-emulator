use core::cell::RefCell;
use std::rc::Rc;

use bitflags::bitflags;

use crate::cart::Mirroring;

pub type CartMapper = Rc<RefCell<dyn Mapper>>;
pub trait Mapper {
    // Default NRom PRG banking
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        // if it only has 16KiB, then mirror first bank
        if prg.len() == PRG_BANK_SIZE { prg[addr % (PRG_BANK_SIZE)] }
        else { prg[addr] }
    }
    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}

    // Default NRom CHR banking
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[addr]
    }
    fn write_chr(&mut self, chr: &mut[u8], addr: usize, val: u8) {
        chr[addr] = val;
    }

    fn mirroring(&self) -> Option<Mirroring> { None }
}

pub fn new_mapper_from_id(id: u8) -> Result<CartMapper, String> {
    let mapper: CartMapper = match id {
        0 => Rc::new(RefCell::new(NRom)),
        1 => Rc::new(RefCell::new(Mmc1::default())),
        2 => Rc::new(RefCell::new(UxRom::default())),
        3 => Rc::new(RefCell::new(INesMapper003::default())),
        // 4 => Rc::new(RefCell::new(Mmc3))
        // NOT WORKING
        // 7 => Rc::new(RefCell::new(AxRom::default())),
        // NOT WORKING
        // 9 => Rc::new(RefCell::new(Mmc2::default())),
        //10 => Rc::new(RefCell::new(Mmc4::default())),
        11 => Rc::new(RefCell::new(ColorDreams::default())),
        66 => Rc::new(RefCell::new(GxRom::default())),
        _ => return Err(format!("Mapper {id} not implemented"))
    };

    Ok(mapper)
}

const PRG_BANK_SIZE: usize = 16*1024; // 16 KiB
const CHR_BANK_SIZE: usize = 8*1024; // 8 KiB

const FIRST_PRG_BANK_END: usize = PRG_BANK_SIZE-1;
const SECOND_PRG_BANK_START: usize = PRG_BANK_SIZE;
const SECOND_PRG_BANK_END: usize = PRG_BANK_SIZE*2-1;

// enum PrgBank { First, Second }
// fn map_prg(addr: usize) -> PrgBank {
//     match addr {
//         (0..=FIRST_PRG_BANK_END) => PrgBank::First,
//         (SECOND_PRG_BANK_START..=SECOND_PRG_BANK_END) => PrgBank::Second,
//         _ => unreachable!("addr can't be bigger than ROM size"),
//     }
// }

pub struct Dummy;
impl Mapper for Dummy {
    fn read_prg(&self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn read_chr(&self, _chr: &[u8], _addr: usize) -> u8 { 0 }
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
            2 => Mirroring::Horizontally,
            3 => Mirroring::Vertically,
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
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        match self.ctrl.prg_mode() {
            Mmc1PrgMode::Bank32kb => {
                let bank_start = (self.prg_bank_select >> 1) * PRG_BANK_SIZE*2;
                prg[bank_start + (addr % (PRG_BANK_SIZE*2))]
            }
            Mmc1PrgMode::BankLast16kb => {
                let bank_select = match addr {
                    0..=0x3FFF => 0,
                    0x4000..=0x7FFF => self.prg_bank_select * PRG_BANK_SIZE,
                    _ => unreachable!()
                };
                prg[bank_select + (addr % PRG_BANK_SIZE)]
            }
            Mmc1PrgMode::BankFirst16kb => {
                let bank_select = match addr {
                    0..=0x3FFF => self.prg_bank_select * PRG_BANK_SIZE,
                    0x4000..=0x7FFF => prg.len() - PRG_BANK_SIZE,
                    _ => unreachable!()
                };
                prg[bank_select + (addr % PRG_BANK_SIZE)]
            }
        }
    }

    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        match self.ctrl.chr_mode() {
            Mmc1ChrMode::Bank8kb => {
                let bank_start = (self.chr_bank0_select >> 1) * CHR_BANK_SIZE;
                chr[bank_start + (addr % CHR_BANK_SIZE)]
            }
            Mmc1ChrMode::Bank4kb => {
                let bank_select = match addr {
                    0..=0x0FFF => self.chr_bank0_select,
                    0x1000..=0x1FFF => self.chr_bank1_select,
                    _ => unreachable!()
                };

                chr[bank_select * CHR_BANK_SIZE/2 + (addr % (CHR_BANK_SIZE/2))]
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

// Mapper 4 https://www.nesdev.org/wiki/MMC3
pub struct Mmc3;
impl Mapper for Mmc3 {}

// Mapper 2 https://www.nesdev.org/wiki/UxROM
#[derive(Default)]
pub struct UxRom {
    prg_bank_select: usize,
}
impl Mapper for UxRom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        if (SECOND_PRG_BANK_START..=SECOND_PRG_BANK_END).contains(&addr) {
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
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
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

// TODO: NOT WORKING
impl Mapper for AxRom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + (addr % PRG_BANK_SIZE*2)]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        // Banks are 32kb big, not 16kB!
        self.prg_bank_select = (val & 0b0000_0111) as usize * PRG_BANK_SIZE*2;
        
        self.mirroring_page = match val & 0b0001_0000 != 0 {
            false => Mirroring::SingleScreenFirstPage,
            true => Mirroring::SingleScreenSecondPage,
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
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + (addr % (PRG_BANK_SIZE*2))]
    }

    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + (addr % CHR_BANK_SIZE)]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize * CHR_BANK_SIZE;
        // Banks are 32kb big, not 16kB!
        self.prg_bank_select = ((val & 0b0011_0000) >> 4) as usize * PRG_BANK_SIZE*2;
    }
}

// Mapper 9 https://www.nesdev.org/wiki/MMC2
#[derive(Default)]
pub struct Mmc2 {
    pub prg_bank_select: usize,
    pub chr_bank_select: usize,
    pub mirroring: Mirroring,
}
impl Mapper for Mmc2 {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        // last three 8kb prg banks
        if (0x2000..=SECOND_PRG_BANK_END).contains(&addr) {
            let last_three_banks = prg.len() - CHR_BANK_SIZE*3;
            prg[last_three_banks + (addr - CHR_BANK_SIZE*3)]
        } else {
            prg[self.prg_bank_select + (addr % (CHR_BANK_SIZE*3))]
        }
    }

    fn read_chr(&self, _chr: &[u8], addr: usize) -> u8 {
        // last two switchable chr banks
        if (0x1000..=0x1FFF).contains(&addr) {
            0
        } else {
            0
        }
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {
        
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
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + (addr % (PRG_BANK_SIZE*2))]
    }

    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + (addr % CHR_BANK_SIZE)]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        // Banks are 32kb big, not 16kB!
        self.prg_bank_select = (val & 0b0000_0011) as usize * PRG_BANK_SIZE*2;
        self.chr_bank_select = ((val & 0b1111_0000) >> 4) as usize * CHR_BANK_SIZE;
    }
}