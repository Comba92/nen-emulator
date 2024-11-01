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

use std::{cell::{RefCell, RefMut}, fmt::Write};

pub const MEM_SIZE: usize = 0x1_0000; // 64KB

const STACK_START: u16 = 0x0100;
const STACK_END: u16 = CPU_RAM_START-1;

const CPU_RAM_START: u16 = 0x0200;
const CPU_RAM_SIZE: u16 = 0x0600; // 

const RAM_START: u16 = 0x0000;
const RAM_SIZE: u16 = 0x0800;
const RAM_END: u16 = RAM_MIRROR_START-1;

const RAM_MIRROR_START: u16 = 0x0800;
const RAM_MIRROR_SIZE: u16 = 0x0800;
const RAM_MIRRORS_END: u16 = 0x1FFF;

const PPU_REG_MIRRORS_START: u16 = 0x2008;
const PPU_REG_MIRRORS_SIZE: u16 = 0x1FF8;

const CART_MEM_START: u16 = 0x4020;
const CART_MEM_SIZE: u16 = 0xBFE0;
const CART_RAM_START: u16 = 0x6000;
const CART_RAM_SIZE: u16 = 0x2000;
const CART_ROM_START: u16 = 0x8000;
const CART_ROM_SIZE: u16 = 0x8000;
const CART_MEM_END: u16 = 0xFFFF;

trait MemAccess {
    fn mem_read(&mut self, addr: u16) -> u8;
    fn mem_read16(&mut self, add: u16) -> u16;
    fn mem_write(&mut self, addr: u16, val: u8);
    fn mem_write16(&mut self, addr: u16, val: u16);
}

pub struct Bus {
    pub mem: RefCell<[u8; MEM_SIZE as usize]>,
}

impl Bus {
    fn mem_access(&self, addr: u16, write: Option<u8>) -> Option<u8> {
        let dst = match addr {
            0..=RAM_END => addr,
            RAM_MIRROR_START..=RAM_MIRRORS_END => {
                (RAM_MIRROR_START - addr) % RAM_MIRROR_SIZE
            },
            CART_MEM_START..=CART_MEM_END => addr,
            _ => unimplemented!("can't access address ${addr:04X}"),
        };

        match write {
            None => Some(self.mem.borrow()[dst as usize]),
            Some(val) => {
                self.mem.borrow_mut()[dst as usize] = val;
                None 
            }
        }
    }

    pub fn mem_read(&self, addr: u16) -> u8 {
        self.mem_access(addr, None).unwrap()
    }

    pub fn mem_write(&self, addr: u16, val: u8) {
        self.mem_access(addr, Some(val));
    }
}
