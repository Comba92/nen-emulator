use std::{cell::RefCell, rc::Rc};

pub type CartMapper = Rc<RefCell<dyn Mapper>>;
pub trait Mapper {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8;
    fn write_prg(&mut self, _prg: &[u8], _addr: usize, _val: u8) {}
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8;
    fn write_chr(&mut self, _chr: &[u8], _addr: usize, _val: u8) {}
}

pub struct Nrom;
impl Mapper for Nrom {
    fn read_prg(&self, prg: &[u8], addr: usize) -> u8 {
        if prg.len() == 16 * 1024 { prg[addr & 0xBFFF] }
        else { prg[addr] }
    }
    fn read_chr(&self, chr: &[u8], addr: usize) -> u8 { chr[addr as usize] }
}

pub fn new_mapper_from_id(id: u8) -> Rc<RefCell<dyn Mapper>> {
    let mapper = match id {
        0 => Nrom,
        _ => panic!("Mapper not implemented, game can't be loaded correctly")
    };

    Rc::new(RefCell::new(mapper))
}