//  _______________ $10000  _______________
// | PRG-ROM       |       |               |
// | Upper Bank    |       |               |
// |_ _ _ _ _ _ _ _| $C000 | PRG-ROM       |
// | PRG-ROM       |       |               |
// | Lower Bank    |       |               |
// |_______________| $8000 |_______________|
// | SRAM          |       | SRAM          |
// |_______________| $6000 |_______________|
// | Expansion ROM |       | Expansion ROM |
// |_______________| $4020 |_______________|
// | I/O Registers |       |               |
// |_ _ _ _ _ _ _ _| $4000 |               |
// | Mirrors       |       | I/O Registers |
// | $2000-$2007   |       |               |
// |_ _ _ _ _ _ _ _| $2008 |               |
// | I/O Registers |       |    ( PPU )    |
// |_______________| $2000 |_______________|
// | Mirrors       |       |               |
// | $0000-$07FF   |       |               |
// |_ _ _ _ _ _ _ _| $0800 |               |
// | RAM           |       | RAM           |
// |_ _ _ _ _ _ _ _| $0200 |               |
// | Stack         |       |               |
// |_ _ _ _ _ _ _ _| $0100 |               |
// | Zero Page     |       |               |
// |_______________| $0000 |_______________|
#![allow(dead_code)]

use std::cell::{OnceCell, RefCell, RefMut};

use log::{info, trace, warn};

use super::{cart::Cart, ppu::Ppu};

pub const MEM_SIZE: usize = 0x1_0000; // 64KB

const STACK_START: u16 = 0x0100;
const STACK_END: u16 = RAM_START-1;

const RAM_START: u16 = 0x0200;
const RAM_SIZE: usize = 0x0600;

const WRAM_START: u16 = 0x0000;
const WRAM_SIZE: usize = 0x0800; // 2KB
const WRAM_END: u16 = RAM_MIRROR_START-1;

const RAM_MIRROR_START: u16 = 0x0800;
const RAM_MIRROR_SIZE: u16 = 0x0800;
const RAM_MIRRORS_END: u16 = 0x1FFF;

pub const PPU_REG_START: u16 = 0x2000;
pub const PPU_REG_END: u16 = 0x2007;

const PPU_REG_MIRRORS_START: u16 = 0x2008;
const PPU_REG_MIRRORS_END: u16 = 0x3FFF;
const PPU_REG_MIRRORS_SIZE: u16 = 0x1FF8;

const CART_MEM_START: u16 = 0x4020;
const CART_MEM_SIZE: usize = 0xBFE0;
const SRAM_START: u16 = 0x6000;
const SRAM_SIZE: u16 = 0x2000;
const ROM_START: u16 = 0x8000;
const ROM_SIZE: usize = 0x8000;
const CART_MEM_END: u16 = 0xFFFF;

trait Device {
    // fn mem_read(&mut self, addr: u16) -> u8;
    // fn mem_read16(&mut self, add: u16) -> u16;
    // fn mem_write(&mut self, addr: u16, val: u8);
    // fn mem_write16(&mut self, addr: u16, val: u16);
}

#[derive(Debug)]
pub enum BusTarget {
    Ram, SRam, Rom, Ppu, None
}

pub struct Bus {
    pub ram: RefCell<[u8; WRAM_SIZE]>,
    sram: RefCell<[u8; CART_MEM_SIZE]>,
    rom: RefCell<[u8; ROM_SIZE]>,

    ppu: OnceCell<RefCell<Ppu>>,
}

impl Bus {
    pub fn new(cart: &Cart) -> Self {
        let bus = Self { 
            ram: RefCell::new([0; WRAM_SIZE as usize]),
            ppu: OnceCell::new(),
            sram: RefCell::new([0; CART_MEM_SIZE]),
            rom: RefCell::new([0; ROM_SIZE]),
        };
        bus.write_data(0x8000, &cart.prg_rom);
        bus.write_data(0xC000, &cart.prg_rom);
        bus
    }

    pub fn connect_ppu(&self, ppu: Ppu) {
        self.ppu.set(RefCell::new(ppu)).unwrap();
    }

    pub fn with_ppu(cart: &Cart) -> Self {
        let bus = Bus::new(cart);
        bus.connect_ppu(Ppu::new(cart));
        bus
    }

    pub fn step(&self, cycles: usize, cpu_cycles: usize) {
        self.ppu().step(cycles * 3, cpu_cycles);
    }

    pub fn ppu(&self) -> RefMut<Ppu> {
        self.ppu.get().unwrap().borrow_mut()
    }

    pub fn poll_nmi(&self) -> bool {
        let mut ppu = self.ppu();
        let nmi = ppu.nmi_requested;
        ppu.nmi_requested = false;
        nmi
    }

    // TODO IRQ
    pub fn poll_irq(&self) -> bool { false }

    pub fn map(&self, addr: u16) -> (BusTarget, u16) {
        match addr {
            0..=RAM_MIRRORS_END => {
                let ram_addr = addr & WRAM_END;
                (BusTarget::Ram, ram_addr)
            },
            PPU_REG_START..=PPU_REG_END => {
                warn!("Access to PPU REG ${addr:04X}");
                (BusTarget::Ppu, addr)
            },
            PPU_REG_MIRRORS_START..=PPU_REG_MIRRORS_END => {
                let mirrored = addr & PPU_REG_END;
                warn!("Access to PPU REG ${mirrored:04X} (original: ${addr:04X})");
                (BusTarget::Ppu, mirrored)
            }
            CART_MEM_START..ROM_START => {
                let rom_addr = addr - CART_MEM_START;
                (BusTarget::SRam, rom_addr)
            }
            ROM_START..=CART_MEM_END => {
                let rom_addr = addr - ROM_START;
                (BusTarget::Rom, rom_addr)
            },
            _ => {
                info!("Access to ${addr:04X} not implemented");
                (BusTarget::None, 0)
            },
        }
    }


    pub fn read(&self, addr: u16) -> u8 {
        let (target, new_addr) = self.map(addr);
        trace!("Sent: {addr}, Got {new_addr}, Target: {target:?}");
        match target {
            BusTarget::Ram => self.ram.borrow()[new_addr as usize],
            BusTarget::Ppu => self.ppu().reg_read(new_addr),
            BusTarget::SRam => self.sram.borrow()[new_addr as usize],
            BusTarget::Rom => self.rom.borrow()[new_addr as usize],
            BusTarget::None => 0,
        }
    }

    pub fn read16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
    }

    pub fn write(&self, addr: u16, val: u8) {
        let (target, new_addr) = self.map(addr);
        match target {
            BusTarget::Ram => self.ram.borrow_mut()[new_addr as usize] = val,
            BusTarget::Ppu => self.ppu().reg_write(new_addr, val),
            BusTarget::SRam => self.sram.borrow_mut()[new_addr as usize] = val,
            BusTarget::Rom => self.rom.borrow_mut()[new_addr as usize] = val,
            BusTarget::None => {},
        }
    }

    pub fn write16(&self, addr: u16, val: u16) {
        let [low, high] = val.to_le_bytes();
        self.write(addr, low);
        self.write(addr.wrapping_add(1), high);
    }

    pub fn write_data(&self, start: u16, data: &[u8]) {
        for (offset, byte) in data.iter().enumerate() {
            self.write(start.wrapping_add(offset as u16), *byte);
        }
    }
}

#[cfg(test)]
mod bus_tests {
    use crate::emu::ppu::{PPU_ADDR, PPU_CTRL, PPU_DATA, PPU_MASK, PPU_SCROLL, PPU_STAT};

    use super::*;

    fn new_bus() -> Bus {
        colog::init();
        let bus = Bus::new(&Cart::empty());
        bus.connect_ppu(Ppu::new(&Cart::empty()));
        bus
    }

    #[test]
    fn ppu_regs() {
        let bus = new_bus();
        bus.read(PPU_CTRL);
        bus.read(PPU_MASK);
        bus.read(PPU_STAT);
        bus.read(PPU_SCROLL);
        bus.read(PPU_ADDR);
        bus.read(PPU_DATA);
    }

    #[test]
    fn ppu_reg_mirror() {
        let bus = new_bus();
        bus.ppu().vram[0x2121] = 0x69;

        bus.read(0x351E);
        bus.write(0x351E, 0x21);
        bus.write(0x351E, 0x21);
        bus.read(0x351F);
        assert_eq!(bus.read(0x351F), 0x69);
    }

    #[test]
    fn ppu_addr() {
        let bus = new_bus();
        bus.ppu().vram[0x05] = 0x69;

        bus.write(PPU_ADDR, 0x20);
        bus.write(PPU_ADDR, 0x05);

        bus.read(PPU_DATA);
        assert_eq!(bus.read(PPU_DATA), 0x69);
    }
}
