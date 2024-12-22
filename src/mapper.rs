use core::cell::RefCell;
use std::rc::Rc;
mod mmc1;
mod uxrom;
mod inesmapper3;
mod mmc3;
mod axrom;
mod mmc2;
mod colordreams;
mod gxrom;
mod vrc4;

use axrom::AxRom;
use colordreams::ColorDreams;
use gxrom::GxRom;
use inesmapper3::INesMapper003;
use mmc1::Mmc1;
use mmc2::Mmc2;
use mmc3::Mmc3;
use uxrom::UxRom;
use vrc4::Vrc4;

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
    fn read_sram(&mut self, _sram: &[u8], _addr: usize) -> u8 { 0 }

    fn write_prg(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}
    fn write_chr(&mut self, chr: &mut[u8], addr: usize, val: u8) { chr[addr] = val; }
    fn write_sram(&mut self, _sram: &mut[u8], _addr: usize, _val: u8) {}

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
    fn notify_cpu_cycle(&mut self) {}
    fn notify_scanline(&mut self) {}
    fn poll_irq(&mut self) -> bool { false }
}

pub fn new_mapper_from_id(id: u8) -> Result<CartMapper, String> {
    let mapper: CartMapper = match id {
        0  => Rc::new(RefCell::new(NRom)),
        1  => Rc::new(RefCell::new(Mmc1::default())),
        2  => Rc::new(RefCell::new(UxRom::default())),
        3  => Rc::new(RefCell::new(INesMapper003::default())),
        4  => Rc::new(RefCell::new(Mmc3::default())),
        // 5 => // TODO, most complex one
        // https://www.nesdev.org/wiki/MMC5
        7  => Rc::new(RefCell::new(AxRom::default())),
        9  => Rc::new(RefCell::new(Mmc2::default())),
        11 => Rc::new(RefCell::new(ColorDreams::default())),
        21 | 22 | 23 | 25 => Rc::new(RefCell::new(Vrc4::default())),
        // 64 => // TODO, 5 games
        // https://www.nesdev.org/wiki/RAMBO-1
        66 => Rc::new(RefCell::new(GxRom::default())),
        // 69 => // TODO, this only plays Batman: Return of the Joker
        // https://www.nesdev.org/wiki/Sunsoft_FME-7
        _ => return Err(format!("Mapper {id} not implemented"))
    };

    Ok(mapper)
}

const SRAM_START: usize = 0x6000;
const ROM_START: usize  = 0x8000;
const DEFAULT_PRG_BANK_SIZE: usize = 16*1024; // 16 KiB
const DEFAULT_CHR_BANK_SIZE: usize = 8*1024; // 8 KiB

pub struct Dummy;
impl Mapper for Dummy {
    fn read_prg(&mut self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn read_chr(&mut self, _chr: &[u8], _addr: usize) -> u8 { 0 }
}

// Mapper 0 https://www.nesdev.org/wiki/NROM
struct NRom;
impl Mapper for NRom {}
