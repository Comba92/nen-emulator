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
use vrc4::Vrc2_4;

use crate::cart::Mirroring;

pub trait Mapper {
    // Default NRom PRG banking
    fn prg_addr(&mut self, prg: &[u8], addr: usize) -> usize {
        // if it only has 16KiB, then mirror to first bank
        if prg.len() == self.prg_bank_size() { 
            self.prg_bank_addr(prg, 0, addr)
        }
        else { addr - ROM_START }
    }
    
    // Default NRom CHR banking
    fn chr_addr(&mut self, _chr: &[u8], addr: usize) -> usize { addr }
    fn sram_addr(&mut self, _sram: &[u8], addr: usize) -> usize { addr }

    fn prg_read(&mut self, prg: &[u8], addr: usize) -> u8 { prg[self.prg_addr(prg, addr)] }
    fn chr_read(&mut self, chr: &[u8], addr: usize) -> u8 { chr[self.chr_addr(chr, addr)] }

    fn sram_read(&mut self, sram: &[u8], addr: usize) -> u8 { todo!() }

    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, _val: u8);
    fn chr_write(&mut self, chr: &mut[u8], addr: usize, val: u8) { chr[self.chr_addr(chr, addr)] = val; }
    fn sram_write(&mut self, sram: &mut[u8], addr: usize, val: u8) { todo!() }

    fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE }
    fn chr_bank_size(&self) -> usize { DEFAULT_CHR_BANK_SIZE }

    fn prg_banks_count(&self, prg: &[u8]) -> usize { prg.len() / self.prg_bank_size() }
    fn chr_banks_count(&self, chr: &[u8]) -> usize { chr.len() / self.chr_bank_size() }
    
    fn prg_last_bank(&self, prg: &[u8]) -> usize { self.prg_banks_count(prg) - 1 }

    fn prg_bank_addr(&self, prg: &[u8], bank: usize, addr: usize) -> usize {
        let bank_start = (bank % self.prg_banks_count(prg)) * self.prg_bank_size();
        let offset = (addr - ROM_START) % self.prg_bank_size();
        bank_start + offset
    }

    fn chr_bank_addr(&self, chr: &[u8], bank: usize, addr: usize) -> usize {
        let bank_start = (bank % self.chr_banks_count(chr)) * self.chr_bank_size();
        let offset = addr % self.chr_bank_size();
        bank_start + offset
    }

    fn mirroring(&self) -> Option<Mirroring> { None }
    fn notify_cpu_cycle(&mut self) {}
    fn notify_scanline(&mut self) {}
    fn poll_irq(&mut self) -> bool { false }
}

pub type CartMapper = Box<dyn Mapper>;
pub fn new_mapper_from_id(id: u8) -> Result<CartMapper, String> {
    let mapper: CartMapper = match id {
        0  => Box::new(NRom),
        1  => Box::new(Mmc1::default()),
        2  => Box::new(UxRom::default()),
        3  => Box::new(INesMapper003::default()),
        4  => Box::new(Mmc3::default()),
        // 5 => // TODO, most complex one
        // https://www.nesdev.org/wiki/MMC5
        7  => Box::new(AxRom::default()),
        9  => Box::new(Mmc2::default()),
        11 => Box::new(ColorDreams::default()),
        21 | 22 | 23 | 25 => Box::new(Vrc2_4::new(id)),
        // 64 => // TODO, 5 games
        // https://www.nesdev.org/wiki/RAMBO-1
        66 => Box::new(GxRom::default()),
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
    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}
    fn prg_read(&mut self, _prg: &[u8], _addr: usize) -> u8 { 0 }
    fn chr_read(&mut self, _chr: &[u8], _addr: usize) -> u8 { 0 }
}

// Mapper 0 https://www.nesdev.org/wiki/NROM
struct NRom;
impl Mapper for NRom {
    fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}
}
