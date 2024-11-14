use std::{cell::RefCell, rc::Rc};

pub type CartMapper = Rc<RefCell<dyn Mapper>>;
pub trait Mapper {
    // Default NRom PRG banking
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        if prg.len() == PRG_BANK_SIZE { prg[addr % (PRG_BANK_SIZE)] }
        else { prg[addr] }
    }
    fn write_prg(&mut self, _addr: usize, _val: u8) {}

    // Default NRom CHR banking
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[addr]
    }
    fn write_chr(&mut self, _addr: usize, _val: u8) {}
}

pub fn new_mapper_from_id(id: u8) -> CartMapper {
    match id {
        0 => Rc::new(RefCell::new(NRom)),
        1 => Rc::new(RefCell::new(Mmc1)),
        2 => Rc::new(RefCell::new(UxRom::default())),
        3 => Rc::new(RefCell::new(INesMapper003::default())),

        _ => panic!("Mapper {id} not implemented, game can't be loaded correctly")
    }
}

// enum PrgBank { First, Second }
// fn map_prg(addr: usize) -> PrgBank {
//     match addr {
//         0..0x4000      => PrgBank::First,
//         0x4000..0x8000 => PrgBank::Second,
//         _ => unreachable!()
//     }
// }

const PRG_BANK_SIZE: usize = 16*1024;
const CHR_BANK_SIZE: usize = 8*1024;

const SECOND_PRG_BANK_START: usize = PRG_BANK_SIZE;
const SECOND_PRG_BANK_END: usize = PRG_BANK_SIZE*2-1;

pub struct Dummy;
impl Mapper for Dummy {
    fn read_prg(&self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn read_chr(&self, _chr: &[u8], _addr: usize) -> u8 { 0 }
}

pub struct NRom;
impl Mapper for NRom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        if prg.len() == PRG_BANK_SIZE { prg[addr % (PRG_BANK_SIZE)] }
        else { prg[addr] }
    }
}

pub struct Mmc1;
impl Mapper for Mmc1 {}

#[derive(Default)]
pub struct UxRom {
    prg_bank: usize,
}
impl Mapper for UxRom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        if (SECOND_PRG_BANK_START..=SECOND_PRG_BANK_END).contains(&addr) { 
            let last_bank_start = prg.len() - PRG_BANK_SIZE;
            prg[last_bank_start + (addr - PRG_BANK_SIZE)]
        } else {
            prg[self.prg_bank + addr]
        }
    }

    fn write_prg(&mut self, _addr: usize, val: u8) {
        self.prg_bank = (val & 0b0000_1111) as usize * PRG_BANK_SIZE;
    }
}

#[derive(Default)]
pub struct INesMapper003 {
    chr_bank: usize,
}
impl Mapper for INesMapper003 {
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[self.chr_bank + addr]
    }

    fn write_prg(&mut self, _addr: usize, val: u8) {
        self.chr_bank = val as usize * CHR_BANK_SIZE;
    }
}