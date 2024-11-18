use core::cell::RefCell;
use std::rc::Rc;

use serde_json::Map;

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

    fn mirroring(&self) -> Option<usize> { None }
}

pub fn new_mapper_from_id(id: u8) -> Result<CartMapper, String> {
    let mapper: CartMapper = match id {
        0 => Rc::new(RefCell::new(NRom)),
        // 1 => Rc::new(RefCell::new(Mmc1)),
        2 => Rc::new(RefCell::new(UxRom::default())),
        3 => Rc::new(RefCell::new(INesMapper003::default())),
        // 4 => Rc::new(RefCell::new(Mmc3))
        7 => Rc::new(RefCell::new(AxRom::default())),
        //9 => Rc::new(RefCell::new(Mmc2::default())),
        //10 => Rc::new(RefCell::new(Mmc4::default())),
        11 => Rc::new(RefCell::new(ColorDreams::default())),
        66 => Rc::new(RefCell::new(GxRom::default())),
        _ => return Err(format!("Mapper {id} not implemented, game can't be loaded correctly"))
    };

    Ok(mapper)
}

const PRG_BANK_SIZE: usize = 16*1024; // 16 KiB
const CHR_BANK_SIZE: usize = 8*1024; // 8 KiB

const FIRST_PRG_BANK_END: usize = PRG_BANK_SIZE-1;
const SECOND_PRG_BANK_START: usize = PRG_BANK_SIZE;
const SECOND_PRG_BANK_END: usize = PRG_BANK_SIZE*2-1;

enum PrgBank { First, Second }
fn map_prg(addr: usize) -> PrgBank {
    match addr {
        (0..=FIRST_PRG_BANK_END) => PrgBank::First,
        (SECOND_PRG_BANK_START..=SECOND_PRG_BANK_END) => PrgBank::Second,
        _ => unreachable!("addr can't be bigger than ROM size"),
    }
}

pub struct Dummy;
impl Mapper for Dummy {
    fn read_prg(&self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn read_chr(&self, _chr: &[u8], _addr: usize) -> u8 { 0 }
}

pub struct NRom;
impl Mapper for NRom {}

pub struct Mmc1;
impl Mapper for Mmc1 {}

pub struct Mmc3;
impl Mapper for Mmc3 {}

#[derive(Default)]
pub struct UxRom {
    prg_bank_select: usize,
}
impl Mapper for UxRom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        match map_prg(addr) {
            PrgBank::First => prg[self.prg_bank_select + addr],
            PrgBank::Second => {
                let last_bank_start = prg.len() - PRG_BANK_SIZE;
                prg[last_bank_start + (addr - PRG_BANK_SIZE)]
            }
        }
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_1111) as usize * PRG_BANK_SIZE;
    }
}

#[derive(Default)]
pub struct INesMapper003 {
    chr_bank_select: usize,
}
impl Mapper for INesMapper003 {
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + addr]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize * CHR_BANK_SIZE;
    }
}

#[derive(Default)]
pub struct AxRom {
    pub prg_bank_select: usize,
    pub mirroring_page: usize,
}
impl Mapper for AxRom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + addr]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.prg_bank_select = (val & 0b0000_0111) as usize * PRG_BANK_SIZE;
        self.mirroring_page = ((val & 0b0001_0000) >> 4) as usize * 0x2000;
    }

    fn mirroring(&self) -> Option<usize> {
        Some(self.mirroring_page)
    }
}

#[derive(Default)]
pub struct GxRom {
    pub prg_bank_select: usize,
    pub chr_bank_select: usize,
}
impl Mapper for GxRom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + addr]
    }

    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + addr]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize * CHR_BANK_SIZE;
        self.prg_bank_select = ((val & 0b0011_0000) >> 4) as usize * PRG_BANK_SIZE;
    }
}

#[derive(Default)]
pub struct Mmc2 {}
impl Mapper for Mmc2 {

}

#[derive(Default)]
pub struct ColorDreams {
    pub prg_bank_select: usize,
    pub chr_bank_select: usize,
}
impl Mapper for ColorDreams {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        prg[self.prg_bank_select + addr]
    }

    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank_select + addr]
    }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, val: u8) {
        self.chr_bank_select = (val & 0b0000_0011) as usize * CHR_BANK_SIZE;
        self.prg_bank_select = ((val & 0b1111_0000) >> 4) as usize * PRG_BANK_SIZE;
    }
}