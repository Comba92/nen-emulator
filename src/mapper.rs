mod nrom;
mod mmc1;
mod uxrom;
mod inesmapper3;
mod mmc3;
mod axrom;
mod mmc2;
mod colordreams;
mod gxrom;
mod vrc2_4;
mod inesmapper71;
mod mmc5;

use axrom::AxRom;
use colordreams::ColorDreams;
use gxrom::GxRom;
use inesmapper3::INesMapper003;
use inesmapper71::INesMapper071;
use mmc1::Mmc1;
use mmc2::Mmc2;
use mmc3::Mmc3;
use nrom::NRom;
use uxrom::UxRom;
use vrc2_4::Vrc2_4;

use crate::cart::Mirroring;

pub trait Mapper {
    fn prg_addr(&self, _prg: &[u8], addr: usize) -> usize { addr - ROM_START }
    fn chr_addr(&self, _chr: &[u8], addr: usize) -> usize { addr }

    // TODO: open bus behaviour
    fn prg_read(&mut self, prg: &[u8], addr: usize) -> u8 { prg[self.prg_addr(prg, addr)] }
    fn chr_read(&mut self, chr: &[u8], addr: usize) -> u8 { chr[self.chr_addr(chr, addr)] }

    // TODO: open bus behaviour
    fn cart_read(&mut self, _addr: usize) -> u8 { 0 }
    fn cart_write(&mut self, _addr: usize, _val: u8) {}
    fn prg_write(&mut self, prg: &mut[u8], addr: usize, val: u8);
    fn chr_write(&mut self, chr: &mut[u8], addr: usize, val: u8) { chr[self.chr_addr(chr, addr)] = val; }

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
    
    // Mmc3 scanline notify
    fn notify_scanline(&mut self) {}

    // Generic cpu cycle notify
    fn notify_cpu_cycle(&mut self) {}

    // Mmc5 ppu notify
    fn notify_ppuctrl(&mut self, _val: u8) {}
    fn notify_ppumask(&mut self, _val: u8) {}

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
        // 5  => Box::new(Mmc5::default()),
        7  => Box::new(AxRom::default()),
        9  => Box::new(Mmc2::default()),
        11 => Box::new(ColorDreams::default()),
        21 | 22 | 23 | 25 => Box::new(Vrc2_4::new(id)),
        // 64 => // TODO, 5 shitty games
        // https://www.nesdev.org/wiki/RAMBO-1
        66 => Box::new(GxRom::default()),
        // 69 => // TODO, this only plays Batman: Return of the Joker
        // https://www.nesdev.org/wiki/Sunsoft_FME-7
        71 => Box::new(INesMapper071::default()),
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
