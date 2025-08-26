use crate::{cart::{Cart, CartHeader}, emu::{Emu, Mirroring}};

pub trait BankCfg {}

#[derive(Debug)]
pub struct PrgBank;
impl BankCfg for PrgBank {}

#[derive(Debug)]
pub struct ChrBank;
impl BankCfg for ChrBank {}

#[derive(Debug)]
pub struct WramBank;
impl BankCfg for WramBank {}

#[derive(Debug)]
pub struct VramBank;
impl BankCfg for VramBank {}

#[derive(Debug)]
pub struct Banking<T: BankCfg> {
  real_size: usize,
  addressable_size: u16,
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
      real_size,
      bank_size,
      bank_size_shift,
      banks_count,
      addressable_size,

      bankings,
      kind: std::marker::PhantomData::<T>,
    }
  }

  pub fn pages_count(&self) -> usize {
    self.bankings.len()
  }

  pub fn change_mode(&mut self, pages_count: u16) {
    assert!(pages_count as usize <= self.bankings.len());

    // we change the parameters, leaving banks array as is
    // thus we cannot change to a bigger bank size than the original

    // TODO: can this be changed to a shift?
    self.bank_size = self.addressable_size / pages_count;
    self.banks_count = (self.real_size / self.bank_size as usize) as u16;
    self.bank_size_shift = self.bank_size.ilog2() as u16;
  }

  pub fn change_size(&mut self, size: usize) {
    self.real_size = size;
    self.banks_count = (self.real_size / self.bank_size as usize) as u16;
  }

  pub fn set_page(&mut self, page: u8, bank: u8) {
    // some games might write bigger bank numbers than really avaible
    // let bank = bank % self.banks_count;
    let bank = bank as usize & (self.banks_count as usize - 1);

    // i do not expect to write outside the slots array.
    // we precompute the real index instead of keeping the bank number
    // self.bankings[page] = bank * self.bank_size;
    self.bankings[page as usize] = bank << self.bank_size_shift;
  }

  pub fn set_pages_aligned2(&mut self, page: u8, bank: u8) {    
    let bank = bank & !1;
    self.set_page(page, bank);
    self.set_page(page + 1, bank + 1);
  }

  pub fn set_pages_aligned4(&mut self, page: u8, bank: u8) {
    let bank = bank & !0x3;
    for i in 0..4 {
      self.set_page(page + i, bank + i);
    }
  }

  pub fn set_pages_aligned8(&mut self, page: u8, bank: u8) {
    let bank = bank & !0x7;
    for i in 0..8 {
      self.set_page(page + i, bank + i);
    }
  }

  pub fn set_pages_unaligned(&mut self, page: u8, bank: u8, count: u8) {
    for i in 0..count {
      self.set_page(page + i, bank + i);
    }
  }

  pub fn set_page_to_last_bank(&mut self, page: u8) {
    self.set_page(page, self.banks_count as u8-1);
  }

  pub fn swap_pages(&mut self, a: u8, b: u8) {
    self.bankings.swap(a as usize, b as usize);
  }

  pub fn translate(&self, addr: u16) -> usize {
    // let page = (addr % self.pages_size) / self.bank_size;
    // let page = (addr & self.addressable_size-1) >> self.bank_size_shift;
    let page = addr >> self.bank_size_shift;

    // i do not expect to write outside the slots array here either.
    // self.bankings[page] + (addr % self.bank_size)
    // real index + offset
    self.bankings[page as usize & self.bankings.len()-1] + (addr & (self.bank_size - 1)) as usize
  }
}

impl Banking<PrgBank> {
  pub fn new_prg(header: &CartHeader, pages_count: u16) -> Self {
    Self::new(header.prg_size, 32 * 1024, pages_count)
  }
}
impl Default for Banking<PrgBank> {
  fn default() -> Self {
    Banking::new(32 * 1024, 32 * 1024, 2)
  }
}

impl Banking<ChrBank> {
  pub fn new_chr(header: &CartHeader, pages_count: u16) -> Self {
    Self::new(header.chr_size, 8 * 1024, pages_count)
  }
}
impl Default for Banking<ChrBank> {
  fn default() -> Self {
    Banking::new(8 * 1024, 8 * 1024, 1)
  }
}

impl Banking<WramBank> {
  pub fn new_wram(header: &CartHeader) -> Self {
    Self::new(header.wram_size, 8 * 1024, 1)
  }
}
impl Default for Banking<WramBank> {
  fn default() -> Self {
    Banking::new(8* 1024, 8 * 1024, 1)
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
      Mirroring::SingleScreenA => {
        for i in 0..self.bankings.len() {
          self.set_page(i as u8, 0);
        }
      }
      Mirroring::SingleScreenB => {
        for i in 0..self.bankings.len() {
          self.set_page(i as u8, 1);
        }
      }
      Mirroring::FourScreens => {
        for i in 0..self.bankings.len() {
          self.set_page(i as u8, i as u8);
        }
      }
    }
  }
}
impl Default for Banking<VramBank> {
  fn default() -> Self {
    Banking::new(2 * 1024, 1024, 4)
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
      wram: Banking::new_wram(header),
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
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CpuHandler {
  Ram, Ppu, IO, WramRW, WramReadOnly, OpenBus, Prg, PrgInWram, PrgMMC5, PpuMMC5, Mapper,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PpuHandler {
  ChrRom, ChrRam, Vram, Palette, VramInChr,
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

  // TODO: consider keeping this in PPU
  // TODO: palettes are weird, accessing them doesnt change ppu addr bus, investigate
  pub palettes: [u8; 32],

  pub cpu_addr_bus: u16,
  pub cpu_data_bus: u8,
  pub ppu_addr_bus: u16,

  // TODO: remove these, only used for DEBUG porpuoses
  pub ppu_cycle: i16,
  pub ppu_scanline: i16,

  pub nmi: bool,
  pub irq: IrqFlags,

  pub banks: BanksHandler,
}

impl Bus {
  pub fn new(cart: Cart) -> Result<Self, String> {
    let mut banks = BanksHandler::new(&cart.header);
    banks.vram.mirror(&cart.header.mirroring);

    let wram_handler = if cart.header.wram_size == 0 { CpuHandler::OpenBus } else { CpuHandler::WramRW };
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
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Vram,
      PpuHandler::Palette
    ];

    Ok(Self {
      ram: [0; 2 * 1024],
      prg: cart.prg,
      chr: cart.chr,
      vram: vec![0; 2 * 1024],
      wram: vec![0; cart.header.wram_size],
      palettes: [0; 32],

      cpu_handlers_8kb,
      ppu_handlers_1kb,

      cpu_addr_bus: 0,
      cpu_data_bus: 0,
      ppu_addr_bus: 0,
      
      ppu_cycle: 0,
      ppu_scanline: 0,
      
      nmi: false,
      irq: IrqFlags::empty(),

      banks,
    })
  }

  pub fn set_wram_handlers(&mut self, handler: CpuHandler) {
    // only set wram if it is present
    if self.cpu_handlers_8kb[3] != CpuHandler::OpenBus || handler == CpuHandler::Mapper {
      self.cpu_handlers_8kb[3] = handler;
    }
  }

  pub fn set_prg_handlers(&mut self, handler: CpuHandler) {
    for i in 4..8 {
      self.cpu_handlers_8kb[i] = handler;
    }
  }

  pub fn set_vram_handlers(&mut self, handler: PpuHandler) {
    for i in 8..12 {
      self.ppu_handlers_1kb[i] = handler;
    }
  }
}

impl Emu {
  pub fn cpu_dispatch_read(&mut self, addr: u16) -> u8 {
    let mem = &mut self.mem;
    
    // TODO: cpu tick here
    // Be sure to remove ticks in dma and cpu reads

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
          self.mapper.cart_read(mem, addr) | mem.cpu_data_bus 
        }
      }
      CpuHandler::Mapper => self.mapper.cart_read(mem, addr),
      CpuHandler::WramRW | CpuHandler::WramReadOnly => mem.wram[mem.banks.wram.translate(addr - 0x6000)],
      CpuHandler::Prg => mem.prg[mem.banks.prg.translate(addr - 0x8000)],
      CpuHandler::PrgInWram => mem.prg[mem.banks.wram.translate(addr - 0x6000)],
      CpuHandler::PrgMMC5 => {
        self.mapper.notify_cpu_addr(mem, addr, None);
        mem.prg[mem.banks.prg.translate(addr - 0x8000)]
      }
      CpuHandler::OpenBus => mem.cpu_data_bus,
    };
    
    self.mem.cpu_addr_bus = addr;
    self.mem.cpu_data_bus = res;
    res
  }

  pub fn cpu_dispatch_write(&mut self, addr: u16, val: u8) {    
    let mem = &mut self.mem;

    // TODO: cpu tick here
    // Be sure to remove ticks in dma and cpu reads

    let handler = (addr >> 13) % 16;

    match mem.cpu_handlers_8kb[handler as usize] {
      CpuHandler::Ram => mem.ram[addr as usize & 0x07ff] = val,
      CpuHandler::Ppu => self.ppu_reg_write(addr & 0x2007, val),
      CpuHandler::PpuMMC5 => {
        self.mapper.notify_cpu_addr(mem, addr, Some(val));
        self.ppu_reg_write(addr & 0x2007, val);
      }
      CpuHandler::IO => {
        if matches!(addr, 0x4000..=0x4013 | 0x4015 | 0x4017) {
          self.apu_reg_write(addr, val)
        } else if addr == 0x4014 { 
          self.ppu.dma.load((val as u16) << 8, 256)
        } else if addr == 0x4016 { self.joypad.write(val) }
          else { self.mapper.cart_write(mem, addr, val) }
      }
      // 0x4014 => {        
      //   // https://www.nesdev.org/wiki/PPU_registers#OAMDMA_-_Sprite_DMA_($4014_write)
      //   self.cpu_tick();
      //   // TODO: +1 cycle on odd cpu cyles
      //   // TODO: correct DMA behaviour

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
      CpuHandler::WramRW => mem.wram[mem.banks.wram.translate(addr - 0x6000)] = val,
      CpuHandler::Prg | CpuHandler::PrgInWram | CpuHandler::PrgMMC5 => {
        self.mapper.prg_write(mem, addr, val);
      }
      CpuHandler::OpenBus | CpuHandler::WramReadOnly => {},
    }

    self.mem.cpu_addr_bus = addr;
    self.mem.cpu_data_bus = val;
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
          self.ppu_palette_read(addr)
        } else {
          // Video memory's data bus is multiplexed with the low byte of the address bus on pins 31 through 38. Thus a read from an address with no memory connected will usually return the low byte of the address.
          mem.ppu_addr_bus as u8
        }
      }
    };

    // shouldn't set ppu_addr_bus
    if handler != PpuHandler::Palette {
      self.update_ppu_bus(addr);
    }

    res
  }

  pub fn ppu_palette_read(&mut self, addr: u16) -> u8 {
    let pal = addr as usize & 31;
    let res = if pal % 4 == 0 {
      self.mem.palettes[pal & 0xf]
    } else {
      self.mem.palettes[pal]
    };

    res
  }

  pub fn ppu_dispatch_write(&mut self, addr: u16, val: u8) {
    let mem = &mut self.mem;
    
    let addr = addr & 0x3fff;
    let handler = (addr >> 10) % 16;
    
    match mem.ppu_handlers_1kb[handler as usize] {
      PpuHandler::ChrRom => {}
      PpuHandler::ChrRam => mem.chr[mem.banks.chr.translate(addr)] = val,
      PpuHandler::Vram => mem.vram[mem.banks.vram.translate(addr - 0x2000)] = val,
      PpuHandler::VramInChr => mem.vram[mem.banks.vram.translate(addr)] = val,
      PpuHandler::Palette => {
        if matches!(addr, 0x3f00..=0x3fff) {
          let addr = addr as usize & 31;
          let val = val & 0b11_1111;
          
          // if we're writing a transparent color
          if addr % 4 == 0 {
            // write both backdrop colors
            mem.palettes[addr & 0xf] = val;
            mem.palettes[addr & 0xf + 0xf] = val;
          } else {
            // write palette color as is
            mem.palettes[addr] = val;
          }

          // shouldn't set ppu_addr_bus
          return;
        }
      }
    }

    self.update_ppu_bus(addr);
  }
}