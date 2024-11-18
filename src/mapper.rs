use core::cell::RefCell;
use std::rc::Rc;

pub type CartMapper = Rc<RefCell<dyn Mapper>>;
pub trait Mapper {
    // Default NRom PRG banking
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        // if it only has 16KiB, then mirror first bank
        if prg.len() == PRG_BANK_SIZE { prg[addr % (PRG_BANK_SIZE)] }
        else { prg[addr] }
    }
    fn write_prg(&mut self, _addr: usize, _val: u8) {}

    // Default NRom CHR banking
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 {
        chr[addr]
    }
    fn write_chr(&mut self, addr: usize, val: u8) -> (usize, u8) {
        (addr, val)
    }
}

pub fn new_mapper_from_id(id: u8) -> Result<CartMapper, String> {
    let mapper: CartMapper = match id {
        0 => Rc::new(RefCell::new(NRom)),
        // 1 => Rc::new(RefCell::new(Mmc1)),
        2 => Rc::new(RefCell::new(UxRom::default())),
        3 => Rc::new(RefCell::new(INesMapper003::default())),

        _ => return Err(format!("Mapper {id} not implemented, game can't be loaded correctly"))
    };

    Ok(mapper)
}

const PRG_BANK_SIZE: usize = 16*1024; // 16 KiB
const CHR_BANK_SIZE: usize = 8*1024; // 8 KiB

const SECOND_PRG_BANK_START: usize = PRG_BANK_SIZE;
const SECOND_PRG_BANK_END: usize = PRG_BANK_SIZE*2-1;

pub struct Dummy;
impl Mapper for Dummy {
    fn read_prg(&self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn read_chr(&self, _chr: &[u8], _addr: usize) -> u8 { 0 }
}

pub struct NRom;
impl Mapper for NRom {}

pub struct Mmc1;
impl Mapper for Mmc1 {}


// TODO: REQUIRES NES 2.0 FORMAT FOR BIG GAMES
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