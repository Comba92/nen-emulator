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

use super::cart::Cart;

pub const MEM_SIZE: usize = 0x1_0000; // 64KB

const STACK_START: u16 = 0x0100;
const STACK_END: u16 = RAM_START-1;

const RAM_START: u16 = 0x0200;
const RAM_SIZE: u16 = 0x0600;

const WRAM_START: u16 = 0x0000;
const WRAM_SIZE: u16 = 0x0800; // 2KB
const WRAM_END: u16 = RAM_MIRROR_START-1;

const RAM_MIRROR_START: u16 = 0x0800;
const RAM_MIRROR_SIZE: u16 = 0x0800;
const RAM_MIRRORS_END: u16 = 0x1FFF;

const PPU_REG_MIRRORS_START: u16 = 0x2008;
const PPU_REG_MIRRORS_SIZE: u16 = 0x1FF8;

const CART_MEM_START: u16 = 0x4020;
const CART_MEM_SIZE: u16 = 0xBFE0;
const SRAM_START: u16 = 0x6000;
const SRAM_SIZE: u16 = 0x2000;
const ROM_START: u16 = 0x8000;
const ROM_SIZE: u16 = 0x4000;
const CART_MEM_END: u16 = 0xFFFF;

trait MemAccess {
    fn mem_read(&mut self, addr: u16) -> u8;
    fn mem_read16(&mut self, add: u16) -> u16;
    fn mem_write(&mut self, addr: u16, val: u8);
    fn mem_write16(&mut self, addr: u16, val: u16);
}

pub struct Bus {
    mem: RefCell<[u8; MEM_SIZE as usize]>,
}

impl Bus {
    pub fn new(cart: &Cart) -> Self {
        let bus = Self { mem: RefCell::new([0; MEM_SIZE as usize]) };
        bus.write_data(0x8000, &cart.prg_rom);
        bus.write_data(0xC000, &cart.prg_rom);
        bus
    }

    pub fn read(&self, addr: u16) -> u8 {
        self.mem.borrow()[addr as usize]
    }

    pub fn write(&self, addr: u16, val: u8) {
        self.mem.borrow_mut()[addr as usize] = val;
    }

    pub fn write_data(&self, start: u16, data: &[u8]) {
        let mut mem = self.mem.borrow_mut();
        mem[start as usize..start as usize+data.len()].copy_from_slice(data);
    }

    // pub fn new(cart: Cart) -> Self {
    //     Self { ram: [0; WRAM_SIZE as usize], cart_ram: [0; CART_MEM_SIZE as usize], cart }
    // }

    // pub fn read(&self, addr: u16) -> u8 {
    //     match addr {
    //         0..=WRAM_END => self.ram[addr as usize],
    //         RAM_MIRROR_START..=RAM_MIRRORS_END => {
    //             let mirrored = (RAM_MIRROR_START - addr) % RAM_MIRROR_SIZE;
    //             self.ram[mirrored as usize]
    //         },
    //         CART_MEM_START..ROM_START => {
    //             let addr = (addr - CART_MEM_START) % (ROM_START - CART_MEM_START);
    //             self.cart_ram[addr as usize]
    //         }
    //         ROM_START..=CART_MEM_END => {
    //             let rom_addr = (addr - ROM_START);
    //             println!("Original: {addr:04X}, Wrapped: {rom_addr:04X}");
    //             self.cart.prg_rom[rom_addr as usize]
    //         },
    //         _ => {
    //             eprintln!("Read to ${addr:04X} not yet implemented");
    //             0
    //         },
    //     }
    // }

    // pub fn write(&mut self, addr: u16, val: u8) {
    //     match addr {
    //         0..=WRAM_END => self.ram[addr as usize] = val,
    //         RAM_MIRROR_START..=RAM_MIRRORS_END => {
    //             let mirrored = (RAM_MIRROR_START - addr) % RAM_MIRROR_SIZE;
    //             self.ram[mirrored as usize] = val;
    //         },
    //         CART_MEM_START..ROM_START => {
    //             let addr = (addr - CART_MEM_START) % (ROM_START - CART_MEM_START);
    //             self.cart_ram[addr as usize] = val;
    //         }
    //         ROM_START..=CART_MEM_END => eprintln!("Tried to write to ROM at ${addr:04x}"),
    //         _ => eprintln!("Write to ${addr:04X} not yet implemented"),
    //     }
    // }
}
