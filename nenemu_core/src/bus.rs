use crate::{
    emu::{Mirroring, NesEmulator},
    mapper::{self, BoxedMapper, Mapper},
    rom::{self, Cart, Disk, RomData},
};

pub trait BankCfg {}

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default)]
pub struct PrgBank;
impl BankCfg for PrgBank {}
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default)]
pub struct ChrBank;
impl BankCfg for ChrBank {}
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default)]
pub struct WramBank;
impl BankCfg for WramBank {}
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default)]
pub struct VramBank;
impl BankCfg for VramBank {}

#[derive(Debug, Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct Banking<T: BankCfg> {
    pub banks_count: u16,
    banks_count_mask: u16,

    bank_size_mask: u16,
    bank_size_shift: u8,

    start_addr: u16,
    pub bankings: Vec<u32>,
    kind: std::marker::PhantomData<T>,
}

// TODO: this system currently has a problem; the pages_count doesn't take into account the actual size vs addressable size. for example, if 16kb are provided for prg, no mirroring will occur (it has to be set to 2 pages manually)
impl<T: BankCfg + std::fmt::Debug> Banking<T> {
    pub fn new(start_addr: u16, real_size: usize, virt_size: u16, pages_count: u16) -> Self {
        let bankings = vec![0; pages_count as usize];

        let bank_size = virt_size / pages_count;
        // https://stackoverflow.com/questions/25787613/division-and-multiplication-by-power-of-2
        let bank_size_shift = bank_size.ilog2() as u8;
        // TODO: this doesn't work if bankSize is odd!
        let bank_size_mask = bank_size.saturating_sub(1);

        // TODO: if realSize < virtSize and there banks arent big enough, this becomes 0!!
        // TODO: handle unconvetional realSizes (less tha 8KiB)
        let banks_count = (real_size / bank_size as usize) as u16;
        // TODO: this doesn't work if banksCount is odd! (shouldnt happen as it
        // depends on realSize, and it should always be power of two)
        let banks_count_mask = banks_count.saturating_sub(1);

        Self {
            bank_size_shift,
            bank_size_mask,
            banks_count,
            banks_count_mask,
            start_addr,

            bankings,
            kind: std::marker::PhantomData::<T>,
        }
    }

    pub fn set_page(&mut self, page: u8, bank: u16) {
        // some games might write bigger bank numbers than really avaible
        // let bank = bank % self.banks_count;
        // let bank = bank & (self.banks_count-1);
        let bank = bank & self.banks_count_mask;

        // i do not expect to write outside the slots array.
        // we precompute the real index instead of keeping the bank number
        // self.bankings[page] = bank * self.bank_size;
        self.bankings[page as usize] = (bank as u32) << self.bank_size_shift;
    }

    pub fn set_pages_aligned2(&mut self, page: u8, bank: u16) {
        let bank = bank & !1;
        self.set_page(page, bank);
        self.set_page(page + 1, bank | 1);
    }

    pub fn set_pages_aligned4(&mut self, page: u8, bank: u16) {
        let bank = bank & !0x3;
        for i in 0..4 {
            self.set_page(page + i, bank | i as u16);
        }
    }

    pub fn set_pages_aligned8(&mut self, page: u8, bank: u16) {
        let bank = bank & !0x7;
        for i in 0..8 {
            self.set_page(page + i, bank | i as u16);
        }
    }

    pub fn swap_pages(&mut self, a: u8, b: u8) {
        self.bankings.swap(a as usize, b as usize);
    }

    pub fn translate(&self, addr: u16) -> usize {
        // let page = (addr % self.pages_size) / self.bank_size;
        // let page = (addr >> self.bank_size_shift) as usize % self.bankings.len();
        let page = (addr - self.start_addr) >> self.bank_size_shift;

        // i do not expect to write outside the slots array here either.
        // self.bankings[page] + (addr % self.bank_size)
        // real index + offset
        self.bankings[page as usize] as usize | (addr & self.bank_size_mask) as usize
    }
}

impl Banking<PrgBank> {
    pub fn new_prg(header: &RomData, pages_count: u16) -> Self {
        let mut res = Self::new(0x8000, header.prg_size, 32 * 1024, pages_count);
        res.fix_last_page();
        res
    }

    pub fn fix_last_page(&mut self) {
        self.set_page(self.bankings.len() as u8 - 1, self.banks_count - 1);
    }
}

impl Banking<ChrBank> {
    pub fn new_chr(header: &RomData, pages_count: u16) -> Self {
        Self::new(0, header.chr_size, 8 * 1024, pages_count)
    }
}

impl Banking<WramBank> {
    pub fn new_wram(header: &RomData, pages_count: u16) -> Self {
        Self::new(0x6000, header.wram_size, 8 * 1024, pages_count)
    }
}

impl Banking<VramBank> {
    pub fn new_vram(header: &RomData) -> Self {
        let mut res = Self::new(0x2000, 2 * 1024, 4 * 1024, 4);
        res.mirror(&header.mirroring);
        res
    }

    pub fn mirror(&mut self, mirroring: &Mirroring) {
        match mirroring {
            Mirroring::Horizontal => {
                self.set_page(0, 0);
                self.set_page(1, 0);
                self.set_page(2, 1);
                self.set_page(3, 1);
            }
            Mirroring::Vertical => {
                self.set_page(0, 0);
                self.set_page(1, 1);
                self.set_page(2, 0);
                self.set_page(3, 1);
            }
            Mirroring::LowTable => {
                for i in 0..self.bankings.len() {
                    self.set_page(i as u8, 0);
                }
            }
            Mirroring::HighTable => {
                for i in 0..self.bankings.len() {
                    self.set_page(i as u8, 1);
                }
            }
            Mirroring::FourScreens => {
                for i in 0..self.bankings.len() {
                    self.set_page(i as u8, i as u16);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct BanksHandler {
    pub prg: Banking<PrgBank>,
    pub chr: Banking<ChrBank>,
    pub wram: Banking<WramBank>,
    pub vram: Banking<VramBank>,
}
impl BanksHandler {
    pub fn new(header: &RomData) -> Self {
        Self {
            prg: Banking::new_prg(header, 2),
            chr: Banking::new_chr(header, 1),
            wram: Banking::new_wram(header, 1),
            vram: Banking::new_vram(header),
        }
    }
}

bitflags::bitflags! {
  #[derive(Debug, Default, Clone)]
  #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
  pub struct IrqFlags: u8 {
    const FRAME = 1 << 0;
    const DMC = 1 << 2;
    const MAPPER = 1 << 3;
    const DISK = 1 << 4;
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub enum CpuHandler {
    Ram,
    Ppu,
    IO,
    Wram,
    WramReadOnly,
    Prg,
    OpenBus,
    Mapper,
    PpuMMC3,
    PpuMMC5,
    PrgCustom,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub enum PpuHandler {
    ChrRom,
    ChrRam,
    Vram,
    Palette,
    OpenBus,
    ChrMMC2,
    ChrRomMMC3,
    ChrRamMMC3,
    ChrMMC5,
    VramMMC5,
}

const DEFAULT_CPU_MAP: [CpuHandler; 8] = [
    CpuHandler::Ram,
    CpuHandler::Ppu,
    CpuHandler::IO,
    CpuHandler::OpenBus,
    CpuHandler::Prg,
    CpuHandler::Prg,
    CpuHandler::Prg,
    CpuHandler::Prg,
];

const DEFAULT_PPU_MAP: [PpuHandler; 16] = [
    PpuHandler::ChrRom,
    PpuHandler::ChrRom,
    PpuHandler::ChrRom,
    PpuHandler::ChrRom,
    PpuHandler::ChrRom,
    PpuHandler::ChrRom,
    PpuHandler::ChrRom,
    PpuHandler::ChrRom,
    PpuHandler::Vram,
    PpuHandler::Vram,
    PpuHandler::Vram,
    PpuHandler::Vram,
    PpuHandler::Vram,
    PpuHandler::Vram,
    PpuHandler::Vram,
    PpuHandler::Palette,
];

#[cfg(feature = "savestates")]
use serde_big_array::BigArray;

// TODO: access prg, chr, sram, vram with unsafe uncheked get, as index bounds cannot be optimized
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct Bus {
    #[cfg_attr(feature = "savestates", serde(with = "BigArray"))]
    ram: [u8; 2 * 1024],
    #[cfg_attr(feature = "savestates", serde(skip))]
    pub prg: Box<[u8]>,
    pub wram: Box<[u8]>,
    pub chr: Box<[u8]>,
    // this has to be a vec (even if it is 2kb 99% of times), as some games can set it to 4kb
    pub vram: Vec<u8>,

    // 64kb / 4kb = 16
    pub cpu_handlers_8kb: [CpuHandler; 8],
    // 16kb / 1kb = 16
    pub ppu_handlers_1kb: [PpuHandler; 16],

    pub cpu_open_bus: u8,
    pub ppu_open_bus: u16,

    pub nmi: bool,
    pub irq: IrqFlags,

    pub header: RomData,
    pub banks: BanksHandler,
}
impl Default for Bus {
    fn default() -> Self {
        Self {
            ram: [0; 2 * 1024],
            prg: vec![].into_boxed_slice(),
            chr: vec![].into_boxed_slice(),
            vram: vec![],
            wram: vec![].into_boxed_slice(),

            cpu_handlers_8kb: std::array::from_fn(|_| CpuHandler::OpenBus),
            ppu_handlers_1kb: std::array::from_fn(|_| PpuHandler::OpenBus),

            cpu_open_bus: 0,
            ppu_open_bus: 0,

            nmi: false,
            irq: IrqFlags::empty(),

            header: RomData::default(),
            banks: BanksHandler::default(),
        }
    }
}

impl Bus {
    pub fn with_cart(cart: Cart) -> Self {
        let banks = BanksHandler::new(&cart.header);

        let wram_handler = if cart.header.wram_size > 0 {
            CpuHandler::Wram
        } else {
            CpuHandler::Mapper
        };

        let chr_handler = if cart.header.has_chr_ram {
            PpuHandler::ChrRam
        } else {
            PpuHandler::ChrRom
        };

        let mut cpu_handlers_8kb = DEFAULT_CPU_MAP;
        cpu_handlers_8kb[3] = wram_handler;

        let mut ppu_handlers_1kb = DEFAULT_PPU_MAP;
        ppu_handlers_1kb[..8].fill(chr_handler);

        let ram = [0; 2 * 1024];
        // Final Fantasy, River City Ransom, Apple Town Story[5], Impossible Mission II[6] amongst others
        // Use the semi-random contents of RAM on powerup to seed their RNGs.
        // _ = getrandom::fill(&mut ram);

        Self {
            ram,
            prg: cart.prg.into_boxed_slice(),
            chr: cart.chr.into_boxed_slice(),
            vram: vec![0; 2 * 1024],
            wram: vec![0; cart.header.wram_size].into_boxed_slice(),

            cpu_handlers_8kb,
            ppu_handlers_1kb,

            cpu_open_bus: 0,
            ppu_open_bus: 0,

            nmi: false,
            irq: IrqFlags::empty(),

            header: cart.header,
            banks,
        }
    }

    pub fn with_disk(disk: Disk, bios: &[u8]) -> (Self, BoxedMapper) {
        let mut header = RomData::default();
        header.format = rom::HeaderFormat::Fds;
        header.mapper = 20;

        let prg = bios.to_vec().into_boxed_slice();
        let mut banks = BanksHandler::default();

        // keep like this so we can just use the standard prg handler
        banks.prg = Banking::new(0xe000, 8 * 1024, 8 * 1024, 1);
        banks.wram = Banking::new(0x6000, 32 * 1024, 32 * 1024, 1);
        banks.chr = Banking::new(0x0000, 8 * 1024, 8 * 1024, 1);
        banks.vram = Banking::new(0x2000, 2 * 1024, 4 * 1024, 4);
        banks.vram.mirror(&Mirroring::Horizontal);

        let cpu_handlers_8kb = [
            CpuHandler::Ram,
            CpuHandler::Ppu,
            CpuHandler::IO,
            CpuHandler::Wram,
            CpuHandler::Wram,
            CpuHandler::Wram,
            CpuHandler::Wram,
            CpuHandler::PrgCustom,
        ];

        let mut ppu_handlers_1kb = DEFAULT_PPU_MAP;
        ppu_handlers_1kb[..8].fill(PpuHandler::ChrRam);

        let mut mem = Self {
            ram: [0; 2 * 1024],
            prg,
            wram: vec![0; 32 * 1024].into_boxed_slice(),
            chr: vec![0; 8 * 1024].into_boxed_slice(),
            vram: vec![0; 2 * 1024],

            cpu_handlers_8kb,
            ppu_handlers_1kb,

            cpu_open_bus: 0,
            ppu_open_bus: 0,

            nmi: false,
            irq: IrqFlags::empty(),

            header,
            banks,
        };

        let mut fds = mapper::fds::FDS::new(&mut mem);
        fds.disk_inserted = disk.sides_bytes.len() > 0;
        fds.disks = disk.sides_bytes;

        (mem, fds as BoxedMapper)
    }

    pub fn wram_enable(&mut self, cond: bool) {
        if self.wram.is_empty() {
            return;
        }

        if cond {
            self.set_wram_handlers(CpuHandler::Wram);
        } else {
            self.set_wram_handlers(CpuHandler::Mapper);
        }
    }

    pub fn set_wram_handlers(&mut self, handler: CpuHandler) {
        if !self.wram.is_empty() || !matches!(handler, CpuHandler::Wram | CpuHandler::WramReadOnly)
        {
            self.cpu_handlers_8kb[3] = handler;
        }
    }

    pub fn set_prg_handlers(&mut self, handler: CpuHandler) {
        for i in 4..8 {
            self.cpu_handlers_8kb[i] = handler;
        }
    }

    pub fn set_chr_handlers(&mut self, handler: PpuHandler) {
        for i in 0..8 {
            self.ppu_handlers_1kb[i] = handler;
        }
    }

    pub fn set_vram_handlers(&mut self, handler: PpuHandler) {
        for i in 8..12 {
            self.ppu_handlers_1kb[i] = handler;
        }
    }

    pub fn set_4screen_mirroring(&mut self) {
        self.banks.vram = Banking::new(0x2000, 4 * 1024, 4 * 1024, 4);
        self.banks.vram.mirror(&Mirroring::FourScreens);
        // self.vram = vec![0; 4 * 1024].into_boxed_slice();
        self.vram.resize(4 * 1024, 0);
    }
}

impl NesEmulator {
    pub fn cpu_dispatch_read(&mut self, addr: u16) -> u8 {
        let mem = &mut self.mem;

        let handler = (addr >> 13) % 8;
        let res = match mem.cpu_handlers_8kb[handler as usize] {
            CpuHandler::Ram => mem.ram[addr as usize & 0x07ff],
            CpuHandler::Ppu | CpuHandler::PpuMMC5 => self.ppu_reg_read(addr),
            CpuHandler::IO => {
                if matches!(addr, 0x4000..=0x4013 | 0x4015 | 0x4017) {
                    self.apu_reg_read(addr)
                } else if addr == 0x4016 {
                    self.joypad.read() | (mem.cpu_open_bus & 0xe0)
                } else {
                    self.mapper.io_read(mem, addr)
                }
            }
            CpuHandler::Mapper => self.mapper.io_read(mem, addr),
            CpuHandler::Wram | CpuHandler::WramReadOnly => mem.wram[mem.banks.wram.translate(addr)],
            CpuHandler::Prg => mem.prg[mem.banks.prg.translate(addr)],
            CpuHandler::OpenBus => mem.cpu_open_bus,

            CpuHandler::PpuMMC3 => {
                let res = self.ppu_reg_read(addr);
                if [0x2006, 0x2007].contains(&(addr & 0x2007)) {
                    self.mapper
                        .ppu_bus_callback(&mut self.mem, self.ppu.v.into(), self.cpu.cycles);
                }
                res
            }

            CpuHandler::PrgCustom => {
                self.mapper.cpu_bus_callback(mem, addr, None);
                mem.prg[mem.banks.prg.translate(addr)]
            }
        };

        self.mem.cpu_open_bus = res;
        res
    }

    pub fn cpu_dispatch_write(&mut self, addr: u16, val: u8) {
        let mem = &mut self.mem;

        let handler = (addr >> 13) % 8;
        match mem.cpu_handlers_8kb[handler as usize] {
            CpuHandler::Ram => mem.ram[addr as usize & 0x07ff] = val,
            CpuHandler::Ppu => self.ppu_reg_write(addr & 0x2007, val),
            CpuHandler::IO => {
                if matches!(addr, 0x4000..=0x4013 | 0x4015 | 0x4017) {
                    self.apu_reg_write(addr, val)
                } else if addr == 0x4014 {
                    self.ppu.dma = Some((val as u16) << 8);
                } else if addr == 0x4016 {
                    self.joypad.write(val)
                } else {
                    self.mapper.io_write(mem, addr, val)
                }
            }

            // TODO: this could just be prg_write...
            CpuHandler::Mapper => self.mapper.io_write(mem, addr, val),
            CpuHandler::Wram => mem.wram[mem.banks.wram.translate(addr)] = val,
            CpuHandler::Prg => {
                self.mapper.prg_write(mem, addr, val);
            }
            CpuHandler::WramReadOnly | CpuHandler::OpenBus => {}

            CpuHandler::PpuMMC3 => {
                self.ppu_reg_write(addr, val);
                if [0x2006, 0x2007].contains(&(addr & 0x2007)) {
                    self.mapper
                        .ppu_bus_callback(&mut self.mem, self.ppu.v.into(), self.cpu.cycles);
                }
            }
            CpuHandler::PpuMMC5 => {
                self.ppu_reg_write(addr & 0x2007, val);
                self.mapper.cpu_bus_callback(&mut self.mem, addr, Some(val));
            }
            CpuHandler::PrgCustom => {}
        }

        self.mem.cpu_open_bus = val;
    }

    pub fn ppu_debug_read(&mut self, addr: u16) -> u8 {
        let mem = &mut self.mem;

        let addr = addr & 0x3fff;
        let handler_id = (addr >> 10) % 16;
        let handler = mem.ppu_handlers_1kb[handler_id as usize];

        let res = match handler {
            PpuHandler::ChrRom | PpuHandler::ChrRam => mem.chr[mem.banks.chr.translate(addr)],
            PpuHandler::Vram => mem.vram[mem.banks.vram.translate(addr)],
            PpuHandler::Palette => {
                if addr >= 0x3f00 {
                    self.ppu.palettes_read(addr)
                } else {
                    // Video memory's data bus is multiplexed with the low byte of the address bus on pins 31 through 38. Thus a read from an address with no memory connected will usually return the low byte of the address.
                    mem.ppu_open_bus as u8
                }
            }
            PpuHandler::OpenBus => mem.ppu_open_bus as u8,

            PpuHandler::ChrMMC2
            | PpuHandler::ChrRomMMC3
            | PpuHandler::ChrRamMMC3
            | PpuHandler::ChrMMC5 => mem.chr[mem.banks.chr.translate(addr)],

            PpuHandler::VramMMC5 => mem.vram[mem.banks.vram.translate(addr)],
            // PpuHandler::ChrRomMMC5 | PpuHandler::ChrRamMMC5 | PpuHandler::VramMMC5 => self.mapper.ppu_special_read(mem, addr),
        };

        res
    }

    pub fn ppu_dispatch_read(&mut self, addr: u16) -> u8 {
        let mem = &mut self.mem;
        mem.ppu_open_bus = addr;

        let addr = addr & 0x3fff;
        let handler_id = (addr >> 10) % 16;
        let handler = mem.ppu_handlers_1kb[handler_id as usize];

        let res = match handler {
            PpuHandler::ChrRom | PpuHandler::ChrRam => mem.chr[mem.banks.chr.translate(addr)],
            PpuHandler::Vram => mem.vram[mem.banks.vram.translate(addr & 0x2fff)],
            PpuHandler::Palette => {
                if addr >= 0x3f00 {
                    self.ppu.palettes_read(addr)
                } else {
                    // normal vram mirrors read
                    mem.vram[mem.banks.vram.translate(addr & 0x2fff)]
                }
            }
            PpuHandler::OpenBus => mem.ppu_open_bus as u8,

            PpuHandler::ChrMMC2 | PpuHandler::ChrRomMMC3 | PpuHandler::ChrRamMMC3 => {
                self.mapper.ppu_bus_callback(mem, addr, self.cpu.cycles);
                mem.chr[mem.banks.chr.translate(addr)]
            }
            PpuHandler::ChrMMC5 | PpuHandler::VramMMC5 => {
                self.mapper.ppu_bus_callback(mem, addr, self.cpu.cycles);
                self.mapper.ppu_special_read(mem, addr)
            }
        };

        res
    }

    pub fn ppu_dispatch_write(&mut self, addr: u16, val: u8) {
        let mem = &mut self.mem;
        mem.ppu_open_bus = addr;

        let addr = addr & 0x3fff;
        let handler = (addr >> 10) % 16;

        match mem.ppu_handlers_1kb[handler as usize] {
            PpuHandler::ChrRom | PpuHandler::ChrMMC5 | PpuHandler::OpenBus => {}
            PpuHandler::ChrRam => mem.chr[mem.banks.chr.translate(addr)] = val,

            PpuHandler::Vram | PpuHandler::VramMMC5 => {
                self.mapper.ppu_bus_callback(mem, addr, self.cpu.cycles);
                mem.vram[mem.banks.vram.translate(addr & 0x2fff)] = val;
            }
            PpuHandler::Palette => {
                if addr >= 0x3f00 {
                    self.ppu.palettes_write(addr, val);
                } else {
                    // normal vram mirrors write
                    mem.vram[mem.banks.vram.translate(addr & 0x2fff)] = val;
                }
            }

            PpuHandler::ChrMMC2 | PpuHandler::ChrRomMMC3 => {
                self.mapper.ppu_bus_callback(mem, addr, self.cpu.cycles)
            }
            PpuHandler::ChrRamMMC3 => {
                mem.chr[mem.banks.chr.translate(addr)] = val;
                self.mapper.ppu_bus_callback(mem, addr, self.cpu.cycles);
            }
        }
    }
}
