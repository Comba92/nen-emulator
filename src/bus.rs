use crate::{cart::{Cart, CartHeader}, disk::Disk, emu::{Emu, Mirroring}, mapper::{self, BoxedMapper, Mapper}};

pub trait BankCfg {}

#[derive(Debug, Default)]
pub struct PrgBank;
impl BankCfg for PrgBank {}

#[derive(Debug, Default)]
pub struct ChrBank;
impl BankCfg for ChrBank {}

#[derive(Debug, Default)]
pub struct WramBank;
impl BankCfg for WramBank {}

#[derive(Debug, Default)]
pub struct VramBank;
impl BankCfg for VramBank {}

#[derive(Debug, Default)]
pub struct Banking<T: BankCfg> {
  bank_size: u16,
  bank_size_shift: u16,
  pub banks_count: u16,

  pub bankings: Vec<usize>,
  kind: std::marker::PhantomData<T>,
}

impl<T: BankCfg + std::fmt::Debug> Banking<T> {
  pub fn new(real_size: usize, addressable_size: u16, pages_count: u16) -> Self {
    let bankings = vec![0; pages_count as usize];
    let bank_size = addressable_size / pages_count;
    let banks_count = (real_size / bank_size as usize) as u16;

    // https://stackoverflow.com/questions/25787613/division-and-multiplication-by-power-of-2
    let bank_size_shift = bank_size.checked_ilog2().unwrap_or_default() as u16;

    Self {
      bank_size,
      bank_size_shift,
      banks_count,

      bankings,
      kind: std::marker::PhantomData::<T>,
    }
  }

  pub fn set_page(&mut self, page: u8, bank: u16) {
    // some games might write bigger bank numbers than really avaible
    // let bank = bank % self.banks_count;
    let bank = bank as usize & (self.banks_count as usize - 1);

    // i do not expect to write outside the slots array.
    // we precompute the real index instead of keeping the bank number
    // self.bankings[page] = bank * self.bank_size;
    self.bankings[page as usize] = bank << self.bank_size_shift;
  }

  pub fn set_pages_aligned2(&mut self, page: u8, bank: u16) {    
    let bank = bank & !1;
    self.set_page(page, bank);
    self.set_page(page + 1, bank + 1);
  }

  pub fn set_pages_aligned4(&mut self, page: u8, bank: u16) {
    let bank = bank & !0x3;
    for i in 0..4 {
      self.set_page(page + i, bank + i as u16);
    }
  }

  pub fn set_pages_aligned8(&mut self, page: u8, bank: u16) {
    let bank = bank & !0x7;
    for i in 0..8 {
      self.set_page(page + i, bank + i as u16);
    }
  }

  pub fn set_pages_unaligned(&mut self, page: u8, bank: u16, count: u8) {
    for i in 0..count {
      self.set_page(page + i, bank + i as u16);
    }
  }

  pub fn set_page_to_last_bank(&mut self, page: u8) {
    self.set_page(page, self.banks_count-1);
  }

  pub fn swap_pages(&mut self, a: u8, b: u8) {
    self.bankings.swap(a as usize, b as usize);
  }

  pub fn translate(&self, addr: u16) -> usize {
    // let page = (addr % self.pages_size) / self.bank_size;
    // let page = (addr >> self.bank_size_shift) as usize % self.bankings.len();
    let page = (addr >> self.bank_size_shift) as usize;

    // i do not expect to write outside the slots array here either.
    // self.bankings[page] + (addr % self.bank_size)
    // real index + offset
    self.bankings[page] + (addr & (self.bank_size - 1)) as usize
  }
}

impl Banking<PrgBank> {
  pub fn new_prg(header: &CartHeader, pages_count: u16) -> Self {
    Self::new(header.prg_size, 32 * 1024, pages_count)
  }
}

impl Banking<ChrBank> {
  pub fn new_chr(header: &CartHeader, pages_count: u16) -> Self {
    Self::new(header.chr_size, 8 * 1024, pages_count)
  }
}

impl Banking<WramBank> {
  pub fn new_wram(header: &CartHeader, pages_count: u16) -> Self {
    Self::new(header.wram_size, 8 * 1024, pages_count)
  }
}

impl Banking<VramBank> {
  pub fn new_vram(header: &CartHeader) -> Self {
    let mut res = Self::new(2 * 1024, 4 * 1024, 4);
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
pub struct BanksHandler {
  pub prg:  Banking<PrgBank>,
  pub chr:  Banking<ChrBank>,
  pub wram: Banking<WramBank>,
  pub vram: Banking<VramBank>,
}
impl BanksHandler {
  pub fn new(header: &CartHeader) -> Self {
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
  pub struct IrqFlags: u8 {
    const FRAME = 1 << 0;
    const DMC = 1 << 2;
    const MAPPER = 1 << 3;
    const DISK = 1 << 4;
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CpuHandler {
  Ram, Ppu, IO, Wram, WramReadOnly, Prg, Mapper, PrgInWram, PrgMMC5, PpuMMC5
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PpuHandler {
  ChrRom, ChrRam, Vram, Palette, VramInChr, ChrMMC5, VramMMC5
}

// TODO: access prg, chr, sram, vram with unsafe uncheked get, as index bounds cannot be optimized
pub struct Bus {
  ram: [u8; 2 * 1024],
  pub prg: Vec<u8>,
  pub wram: Vec<u8>,
  pub chr: Vec<u8>,
  // this has to be a vec (even if it is 2kb 99% of times), as some games can set it to 4kb
  pub vram: Vec<u8>,

  // 64kb / 4kb = 16
  pub cpu_handlers_8kb: [CpuHandler; 8],
  // 16kb / 1kb = 16
  pub ppu_handlers_1kb: [PpuHandler; 16],

  pub cpu_addr_bus: u16,
  pub cpu_data_bus: u8,
  pub ppu_addr_bus: u16,
  pub ppu_data_bus: u8,

  // TODO: remove these, only used for DEBUG porpuoses
  pub ppu_cycle: i16,
  pub ppu_scanline: i16,

  pub nmi: bool,
  pub irq: IrqFlags,

  pub header: CartHeader,
  pub banks: BanksHandler,
}

impl Bus {
  pub fn with_cart(cart: Cart) -> Self {
    let banks = BanksHandler::new(&cart.header);

    let wram_handler = if cart.header.wram_size > 0 { CpuHandler::Wram } else { CpuHandler::Mapper };
    let chr_handler = if cart.header.has_chr_ram { PpuHandler::ChrRam } else { PpuHandler::ChrRom };

    let cpu_handlers_8kb = [
      CpuHandler::Ram,
      CpuHandler::Ppu,
      CpuHandler::IO,
      wram_handler,
      CpuHandler::Prg,
      CpuHandler::Prg,
      CpuHandler::Prg,
      CpuHandler::Prg,
    ];

    let ppu_handlers_1kb = [
      chr_handler,
      chr_handler,
      chr_handler,
      chr_handler,
      chr_handler,
      chr_handler,
      chr_handler,
      chr_handler,
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Palette,
      PpuHandler::Palette,
      PpuHandler::Palette,
      PpuHandler::Palette
    ];

    let ram = [0; 2 * 1024];
    // Final Fantasy, River City Ransom, Apple Town Story[5], Impossible Mission II[6] amongst others
    // Use the semi-random contents of RAM on powerup to seed their RNGs.
    // _ = getrandom::fill(&mut ram);

    Self {
      ram,
      prg: cart.prg,
      chr: cart.chr,
      vram: vec![0; 2 * 1024],
      wram: vec![0; cart.header.wram_size],

      cpu_handlers_8kb,
      ppu_handlers_1kb,

      cpu_addr_bus: 0,
      cpu_data_bus: 0,
      ppu_addr_bus: 0,
      ppu_data_bus: 0,
      
      ppu_cycle: 0,
      ppu_scanline: 0,
      
      nmi: false,
      irq: IrqFlags::empty(),

      header: cart.header,
      banks,
    }
  }

  pub fn with_disk(disk: Disk) -> (Self, BoxedMapper) {
    let mut header = CartHeader::default();
    header.mapper = 20;

    let mut banks = BanksHandler::default();

    // keep like this so we can just use the standard prg handler
    banks.prg = Banking::new(8 * 1024, 32 * 1024, 4);
    banks.prg.set_page_to_last_bank(3);
    let mut prg = vec![0; 8 * 1024];
    prg.copy_from_slice(include_bytes!("../utils/disksys.rom"));
    banks.wram = Banking::new(32 * 1024, 32 * 1024, 1);

    banks.chr = Banking::new(8 * 1024, 8 * 1024, 1);
    banks.vram = Banking::new(2 * 1024, 4 * 1024, 4);
    banks.vram.mirror(&Mirroring::Horizontal);

    let cpu_handlers_8kb = [
      CpuHandler::Ram,
      CpuHandler::Ppu,
      CpuHandler::IO,
      CpuHandler::Wram,
      CpuHandler::Wram,
      CpuHandler::Wram,
      CpuHandler::Wram,
      CpuHandler::Prg,
    ];

    let ppu_handlers_1kb = [
      PpuHandler::ChrRam,
      PpuHandler::ChrRam,
      PpuHandler::ChrRam,
      PpuHandler::ChrRam,
      PpuHandler::ChrRam,
      PpuHandler::ChrRam,
      PpuHandler::ChrRam,
      PpuHandler::ChrRam,
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Palette,
      PpuHandler::Palette,
      PpuHandler::Palette,
      PpuHandler::Palette
    ];

    let mut mem = Self {
      ram: [0; 2 * 1024],
      prg,
      wram: vec![0; 32 * 1024],
      chr: vec![0; 8 * 1024],
      vram: vec![0; 2 * 1024],

      cpu_handlers_8kb,
      ppu_handlers_1kb,

      cpu_addr_bus: 0,
      cpu_data_bus: 0,
      ppu_addr_bus: 0,
      ppu_data_bus: 0,
      
      ppu_cycle: 0,
      ppu_scanline: 0,
      
      nmi: false,
      irq: IrqFlags::empty(),

      header,
      banks,
    };

    let mut fds = mapper::FDS::new(&mut mem);
    fds.disks = disk.sides;

    (mem, fds as BoxedMapper)
  }

  pub fn wram_enable(&mut self, cond: bool) {
    if self.wram.is_empty() { return; }
    
    if cond {
      self.set_wram_handlers(CpuHandler::Wram);
    } else {
      self.set_wram_handlers(CpuHandler::Mapper);
    }
  }

  pub fn set_wram_handlers(&mut self, handler: CpuHandler) {
    if !self.wram.is_empty() || !matches!(handler, CpuHandler::Wram | CpuHandler::WramReadOnly) {
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
    self.banks.vram = Banking::new(4 * 1024, 4 * 1024, 4);
    self.banks.vram.mirror(&Mirroring::FourScreens);
    self.vram.resize(4 * 1024, 0);
  }
}

impl Emu {
  pub fn cpu_dispatch_read(&mut self, addr: u16) -> u8 {
    let mem = &mut self.mem;

    let handler = (addr >> 13) % 16;

    let res = match mem.cpu_handlers_8kb[handler as usize] {
      CpuHandler::Ram => mem.ram[addr as usize & 0x07ff],
      CpuHandler::Ppu | CpuHandler::PpuMMC5 => self.ppu_reg_read(addr & 0x2007),
      CpuHandler::IO => {        
        if matches!(addr, 0x4000..=0x4013 | 0x4015 | 0x4017) {
          self.apu_reg_read(addr)
        } else if addr == 0x4016 {
          self.joypad.read() | (mem.cpu_data_bus & 0xe0)
        } else {
          self.mapper.cart_read(mem, addr)
        }
      }
      CpuHandler::Mapper => self.mapper.cart_read(mem, addr),
      CpuHandler::Wram | CpuHandler::WramReadOnly => mem.wram[mem.banks.wram.translate(addr - 0x6000)],
      CpuHandler::Prg => mem.prg[mem.banks.prg.translate(addr - 0x8000)],
      CpuHandler::PrgInWram => mem.prg[mem.banks.wram.translate(addr - 0x6000)],
      
      CpuHandler::PrgMMC5 => {
        self.mapper.notify_cpu_addr(mem, addr, None);
        mem.prg[mem.banks.prg.translate(addr - 0x8000)]
      }
    };
    
    self.mem.cpu_addr_bus = addr;
    self.mem.cpu_data_bus = res;
    res
  }

  pub fn cpu_dispatch_write(&mut self, addr: u16, val: u8) {    
    let mem = &mut self.mem;

    let handler = (addr >> 13) % 16;

    match mem.cpu_handlers_8kb[handler as usize] {
      CpuHandler::Ram => mem.ram[addr as usize & 0x07ff] = val,
      CpuHandler::Ppu => self.ppu_reg_write(addr & 0x2007, val),
      CpuHandler::IO => {
        if matches!(addr, 0x4000..=0x4013 | 0x4015 | 0x4017) {
          self.apu_reg_write(addr, val)
        } else if addr == 0x4014 { 
          self.ppu.dma.load((val as u16) << 8, 256);
          self.cpu_tick();
        } else if addr == 0x4016 { self.joypad.write(val) }
          else { self.mapper.cart_write(mem, addr, val) }
      }
      // 0x4014 => {        
      //   // https://www.nesdev.org/wiki/PPU_registers#OAMDMA_-_Sprite_DMA_($4014_write)
      //   self.cpu_tick();

      //   let mut addr = (val as u16) << 8;

      //   for _ in 0..256 {
      //     self.cpu_tick();
      //     let byte = self.cpu_dispatch_read(addr);
      //     addr += 1;
      //     self.cpu_tick();
      //     self.ppu.oam_write(byte);
      //   }
      // }

      // TODO: this could just be prg_write...
      CpuHandler::Mapper => self.mapper.cart_write(mem, addr, val),
      CpuHandler::Wram => mem.wram[mem.banks.wram.translate(addr - 0x6000)] = val,
      CpuHandler::Prg | CpuHandler::PrgInWram | CpuHandler::PrgMMC5 => {
        self.mapper.prg_write(mem, addr, val);
      }
      CpuHandler::WramReadOnly => {},

      CpuHandler::PpuMMC5 => {
        self.mapper.notify_cpu_addr(mem, addr, Some(val));
        self.ppu_reg_write(addr & 0x2007, val);
      }
    }

    self.mem.cpu_addr_bus = addr;
    self.mem.cpu_data_bus = val;
  }

  pub fn ppu_debug_read(&mut self, addr: u16) -> u8 {
    let mem = &mut self.mem;
    
    let addr = addr & 0x3fff;
    let handler_id = (addr >> 10) % 16;
    let handler = mem.ppu_handlers_1kb[handler_id as usize];

    let res = match handler {
      PpuHandler::ChrRom | PpuHandler::ChrRam => mem.chr[mem.banks.chr.translate(addr)],
      PpuHandler::Vram => mem.vram[mem.banks.vram.translate(addr - 0x2000)],
      PpuHandler::VramInChr => mem.vram[mem.banks.vram.translate(addr)],
      PpuHandler::Palette => {
        if matches!(addr, 0x3f00..=0x3fff) {
          self.ppu.palettes_read(addr)
        } else {
          // Video memory's data bus is multiplexed with the low byte of the address bus on pins 31 through 38. Thus a read from an address with no memory connected will usually return the low byte of the address.
          mem.ppu_addr_bus as u8
        }
      }
      PpuHandler::ChrMMC5 | PpuHandler::VramMMC5 => self.mapper.special_read(mem, addr),
    };

    res
  }

  pub fn update_ppu_bus(&mut self, addr: u16) {
    self.mem.ppu_addr_bus = addr;
    self.mapper.notify_ppu_addr(&mut self.mem, self.cpu.cycles);
  }

  pub fn ppu_dispatch_read(&mut self, addr: u16) -> u8 {
    let mem = &mut self.mem;
    
    let addr = addr & 0x3fff;
    let handler_id = (addr >> 10) % 16;
    let handler = mem.ppu_handlers_1kb[handler_id as usize];

    let res = match handler {
      PpuHandler::ChrRom | PpuHandler::ChrRam => mem.chr[mem.banks.chr.translate(addr)],
      PpuHandler::Vram => mem.vram[mem.banks.vram.translate(addr - 0x2000)],
      PpuHandler::VramInChr => mem.vram[mem.banks.vram.translate(addr)],
      PpuHandler::Palette => {
        if matches!(addr, 0x3f00..=0x3fff) {
          self.ppu.palettes_read(addr)
        } else {
          // Video memory's data bus is multiplexed with the low byte of the address bus on pins 31 through 38. Thus a read from an address with no memory connected will usually return the low byte of the address.
          mem.ppu_addr_bus as u8
        }
      }
      
      PpuHandler::ChrMMC5 | PpuHandler::VramMMC5 => self.mapper.special_read(mem, addr),
    };

    // shouldn't set ppu_addr_bus
    if handler != PpuHandler::Palette {
      self.update_ppu_bus(addr);
    }

    self.mem.ppu_data_bus = res;

    res
  }

  pub fn ppu_dispatch_write(&mut self, addr: u16, val: u8) {
    let mem = &mut self.mem;

    let addr = addr & 0x3fff;
    let handler = (addr >> 10) % 16;

    match mem.ppu_handlers_1kb[handler as usize] {
      PpuHandler::ChrRom => {}
      PpuHandler::ChrRam | PpuHandler::ChrMMC5 => mem.chr[mem.banks.chr.translate(addr)] = val,
      PpuHandler::Vram | PpuHandler::VramMMC5 => mem.vram[mem.banks.vram.translate(addr - 0x2000)] = val,
      PpuHandler::VramInChr => mem.vram[mem.banks.vram.translate(addr)] = val,
      PpuHandler::Palette => {
        if matches!(addr, 0x3f00..=0x3fff) {
          let addr = addr as usize & 31;
          let val = val & 0x3f;
          
          // if we're writing a transparent color
          if addr % 4 == 0 {
            // write both backdrop colors
            self.ppu.palettes[addr & 0xf] = val;
            self.ppu.palettes[addr & 0xf + 0xf] = val;
          } else {
            // write palette color as is
            self.ppu.palettes[addr] = val;
          }

          // shouldn't set ppu_addr_bus
          return;
        }
      }
    }

    self.update_ppu_bus(addr);
    self.mem.ppu_data_bus = val;
  }
}