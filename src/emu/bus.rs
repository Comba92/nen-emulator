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

use std::cell::RefCell;

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
const ROM_SIZE: usize = 0x4000;
const CART_MEM_END: u16 = 0xFFFF;

trait MemAccess {
    fn mem_read(&mut self, addr: u16) -> u8;
    fn mem_read16(&mut self, add: u16) -> u16;
    fn mem_write(&mut self, addr: u16, val: u8);
    fn mem_write16(&mut self, addr: u16, val: u16);
}

pub struct Bus {
    ram: RefCell<[u8; WRAM_SIZE]>,
    sram: RefCell<[u8; CART_MEM_SIZE]>,
    rom: RefCell<[u8; ROM_SIZE]>,
    
    pub ppu: RefCell<Ppu>,
}

impl Bus {
    pub fn new(cart: &Cart) -> Self {
        let bus = Self { 
            ram: RefCell::new([0; WRAM_SIZE as usize]),
            ppu: RefCell::new(Ppu::new()),
            sram: RefCell::new([0; CART_MEM_SIZE]),
            rom: RefCell::new([0; ROM_SIZE]),
        };

        bus.write_data(0x8000, &cart.prg_rom);
        bus.write_data(0xC000, &cart.prg_rom);
        bus
    }

    pub fn step(&self, cycles: usize) {
        self.ppu.borrow_mut().step(cycles);
    }

    pub fn poll_nmi(&self) -> bool { self.ppu.borrow().nmi_requested }
    // TODO IRQ
    pub fn poll_irq(&self) -> bool { false }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0..=WRAM_END => self.ram.borrow()[addr as usize],
            RAM_MIRROR_START..=RAM_MIRRORS_END => {
                let mirrored = (addr - RAM_MIRROR_START) % RAM_MIRROR_SIZE;
                self.ram.borrow()[mirrored as usize]
            },
            PPU_REG_START..=PPU_REG_END => self.ppu.borrow_mut().reg_read(addr),
            PPU_REG_MIRRORS_START..=PPU_REG_MIRRORS_END => {
                let mirrored = (addr - PPU_REG_MIRRORS_START) % PPU_REG_MIRRORS_SIZE;
                self.read(mirrored)
            }
            CART_MEM_START..ROM_START => {
                let rom_addr = (addr - CART_MEM_START) % (ROM_START - CART_MEM_START);
                self.sram.borrow()[rom_addr as usize]
            }
            ROM_START..=CART_MEM_END => {
                let rom_addr = (addr - ROM_START) % self.rom.borrow().len() as u16;
                self.rom.borrow()[rom_addr as usize]
            },
            _ => {
                eprintln!("Read to ${addr:04X} not yet implemented");
                0
            },
        }
    }

    pub fn read16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read(addr), self.read(addr+1)])
    }

    pub fn write(&self, addr: u16, val: u8) {
        match addr {
            0..=WRAM_END => self.ram.borrow_mut()[addr as usize] = val,
            RAM_MIRROR_START..=RAM_MIRRORS_END => {
                let mirrored = (addr - RAM_MIRROR_START) % RAM_MIRROR_SIZE;
                self.ram.borrow_mut()[mirrored as usize] = val;
            },
            PPU_REG_START..=PPU_REG_END => self.ppu.borrow_mut().reg_write(addr, val),
            PPU_REG_MIRRORS_START..=PPU_REG_MIRRORS_END => {
                let mirrored = (addr - PPU_REG_MIRRORS_START) % PPU_REG_MIRRORS_SIZE;
                self.write(mirrored, val)
            }
            CART_MEM_START..ROM_START => {
                let addr = (addr - CART_MEM_START) % (ROM_START - CART_MEM_START);
                self.sram.borrow_mut()[addr as usize] = val;
            }
            ROM_START..=CART_MEM_END => {
                let rom_addr = (addr - ROM_START) % self.rom.borrow().len() as u16;
                self.rom.borrow_mut()[rom_addr as usize] = val;
            }
            _ => eprintln!("Write to ${addr:04X} not yet implemented"),
        }
    }

    pub fn write16(&self, addr: u16, val: u16) {
        let [low, high] = val.to_le_bytes();
        self.write(addr, low);
        self.write(addr+1, high);
    }

    pub fn write_data(&self, start: u16, data: &[u8]) {
        for (offset, byte) in data.iter().enumerate() {
            self.write(start + offset as u16, *byte);
        }
    }
}
