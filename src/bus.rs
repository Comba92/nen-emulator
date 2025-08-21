use crate::{cart::{Cart, CartHeader}, emu::{Emu, Mirroring}};

bitflags::bitflags! {
  #[derive(Debug, Default, Clone)]
  pub struct IrqFlags: u8 {
    const FRAME = 1 << 0;
    const DMC = 1 << 2;
    const MAPPER = 1 << 3;
  }
}

pub struct MemHandler {
  ram: [u8; 2 * 1024],
  prg: Vec<u8>,
  sram: Vec<u8>,
  chr: Vec<u8>,
  pub vram: [u8; 2 * 1024],

  // TODO: consider keeping this in PPU
  pub palettes: [u8; 32],

  cpu_addr_bus: u16,
  cpu_data_bus: u8,
  ppu_addr_bus: u16,

  pub nmi: bool,
  pub irq: IrqFlags,

  pub banks: BankingHandler,

  // TODO: consider moving this to upper emu struct
}

// TODO: access prg, chr, sram, vram with unsafe uncheked get, as index bounds cannot be optimized

#[derive(Debug, Default)]
pub struct BankingHandler {
  pub prg:  Banking<PrgBank>,
  pub chr:  Banking<ChrBank>,
  pub sram: Banking<SramBank>,
  pub vram: Banking<VramBank>,
}
impl BankingHandler {
  pub fn new(header: &CartHeader) -> Self {
    Self {
      prg: Banking::new_prg(header, 2),
      chr: Banking::new_chr(header, 1),
      sram: Banking::new_sram(),
      vram: Banking::new_vram(header),
    }
  }
}

pub trait BankCfg {}

#[derive(Debug)]
pub struct PrgBank;
impl BankCfg for PrgBank {}

#[derive(Debug)]
pub struct ChrBank;
impl BankCfg for ChrBank {}

#[derive(Debug)]
pub struct SramBank;
impl BankCfg for SramBank {}

#[derive(Debug)]
pub struct VramBank;
impl BankCfg for VramBank {}

#[derive(Debug)]
pub struct Banking<T: BankCfg> {
  data_size: usize,
  data_start: u16,
  pages_size: u16,
  bank_size: u16,
  bank_size_shift: u16,
  pub banks_count: u16,

  pub bankings: Vec<usize>,
  kind: std::marker::PhantomData<T>,
}

impl<T: BankCfg> Banking<T> {
  pub fn new(rom_size: usize, rom_start: u16, pages_size: u16, pages_count: u16) -> Self {
    let bankings = vec![0; pages_count as usize];
    let bank_size = pages_size / pages_count;
    let banks_count = (rom_size / bank_size as usize) as u16;
    // https://stackoverflow.com/questions/25787613/division-and-multiplication-by-power-of-2
    let bank_size_shift = bank_size.checked_ilog2().unwrap_or_default() as u16;

    Self {
      data_size: rom_size,
      data_start: rom_start,
      bank_size,
      bank_size_shift,
      banks_count,
      pages_size,

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
    self.bank_size = self.pages_size / pages_count;
    self.banks_count = (self.data_size / self.bank_size as usize) as u16;
    self.bank_size_shift = self.bank_size.ilog2() as u16;
  }

  pub fn set_page(&mut self, page: u8, bank: u8) {
    // some games might write bigger bank numbers than really avaible
    // let bank = bank % self.banks_count;
    let bank = bank as usize & (self.banks_count as usize - 1);
    // i do not expect to write outside the slots array.
    // self.bankings[page] = bank * self.bank_size;
    self.bankings[page as usize] = bank << self.bank_size_shift;
  }

  pub fn set_page2(&mut self, page: u8, bank: u8) {
    let page = page & !1;
    self.set_page(page, bank);
    self.set_page(page + 1, bank + 1);
  }

  pub fn set_page_to_last_bank(&mut self, page: u8) {
    self.set_page(page, self.banks_count as u8-1);
  }

  pub fn swap_pages(&mut self, a: u8, b: u8) {
    self.bankings.swap(a as usize, b as usize);
  }

  pub fn translate(&self, addr: u16) -> usize {
    // let page = (addr - self.pages_start) / self.bank_size;
    let page = (addr - self.data_start) >> self.bank_size_shift;
    // i do not expect to write outside the slots array here either.
    // the bus object should take responsibilty to always pass correct addresses in range.
    // self.bankings[page] + (addr % self.bank_size)
    self.bankings[page as usize] + (addr & (self.bank_size - 1)) as usize
  }
}

impl Banking<PrgBank> {
  pub fn new_prg(header: &CartHeader, pages_count: u16) -> Self {
    Self::new(header.prg_size, 0x8000, 32 * 1024, pages_count)
  }
}
impl Default for Banking<PrgBank> {
  fn default() -> Self {
    Banking::new(32 * 1024, 0x8000, 16 * 1024, 2)
  }
}

impl Banking<ChrBank> {
  pub fn new_chr(header: &CartHeader, pages_count: u16) -> Self {
    Self::new(header.chr_size, 0, 8 * 1024, pages_count)
  }
}
impl Default for Banking<ChrBank> {
  fn default() -> Self {
    Banking::new(8 * 1024, 0, 8 * 1024, 1)
  }
}

impl Banking<SramBank> {
  pub fn new_sram() -> Self {
    Self::new(8 * 1024, 0x6000, 8 * 1024, 1)
  }
}
impl Default for Banking<SramBank> {
  fn default() -> Self {
    Banking::new(8* 1024, 0x6000, 8 * 1024, 1)
  }
}

impl Banking<VramBank> {
  pub fn new_vram(header: &CartHeader) -> Self {
    let mut res = Self::new(2 * 1024, 0x2000, 4 * 1024, 4);
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
        for i in 0..4 {
          self.set_page(i, 0);
        }
      }
      Mirroring::SingleScreenB => {
        for i in 0..4 {
          self.set_page(i, 1);
        }
      }
      Mirroring::FourScreens => {
        todo!("four screens mirroring");
        // for i in 0..4 {
        //   self.set_page(i, i);
        // }
      }
    }
  }
}
impl Default for Banking<VramBank> {
  fn default() -> Self {
    Banking::new(2 * 1024, 0x2000, 1024, 4)
  }
}

impl MemHandler {
  pub fn new(cart: Cart) -> Result<Self, String> {
    let mut banks = BankingHandler::new(&cart.header);
    banks.vram.mirror(&cart.header.mirroring);

    Ok(Self {
      ram: [0; 2* 1024],
      prg: cart.prg,
      chr: cart.chr,
      vram: [0; 2 * 1024],
      sram: vec![0; 8 * 1024],
      palettes: [0; 32],

      cpu_addr_bus: 0,
      cpu_data_bus: 0,
      ppu_addr_bus: 0,
      
      nmi: false,
      irq: IrqFlags::empty(),

      banks,
    })
  }
}

impl Emu {
  pub fn cpu_dispatch_read(&mut self, addr: u16) -> u8 {
    let mem = &mut self.mem;
    
    // TODO: cpu tick here
    // Be sure to remove ticks in dma and cpu reads

    mem.cpu_addr_bus = addr;
    let res = match addr {
      0x0000..=0x1fff => mem.ram[addr as usize & 0x07ff],
      0x2000..=0x3fff => {
        self.ppu_reg_read(addr & 0x2007)
      }
      0x4000..=0x4013 | 0x4015 | 0x4017 => self.apu_reg_read(addr),
      0x4016 => self.joypad.read() | (self.mem.cpu_data_bus & 0xe0),
      0x6000..=0x7fff => mem.sram[(addr as usize - 0x6000) & 0x1fff],
      0x8000..=0xffff => mem.prg[mem.banks.prg.translate(addr)],
      _ => mem.cpu_data_bus,
    };

    self.mem.cpu_data_bus = res;
    res
  }

  pub fn cpu_dispatch_write(&mut self, addr: u16, val: u8) {    
    let mem = &mut self.mem;

    // TODO: cpu tick here
    // Be sure to remove ticks in dma and cpu reads

    mem.cpu_addr_bus = addr;
    match addr {
      0x0000..=0x1fff => mem.ram[addr as usize & 0x07ff] = val,
      0x2000..=0x3fff => self.ppu_reg_write(addr & 0x2007, val),
      0x4000..=0x4013 | 0x4015 | 0x4017 => self.apu_reg_write(addr, val),
      0x4014 => self.ppu.dma.load((val as u16) << 8, 256),
      0x4016 => self.joypad.write(val),
      0x6000..=0x7fff => mem.sram[(addr as usize - 0x6000) & 0x1fff] = val,
      0x8000..=0xffff => self.mapper.prg_write(mem, addr, val),
      _ => {},
    }
  }

  pub fn ppu_dispatch_read(&mut self, addr: u16) -> u8 {
    let mem = &mut self.mem;
    
    let addr = addr & 0x3fff;
    mem.ppu_addr_bus = addr;
    match addr {
      0x0000..=0x1fff => self.ppu_chr_read(addr),
      0x2000..=0x3eff => self.ppu_vram_read(addr & 0x2fff),
      0x3f00..=0x3fff => self.ppu_palette_read(addr),
      // Video memory's data bus is multiplexed with the low byte of the address bus on pins 31 through 38. Thus a read from an address with no memory connected will usually return the low byte of the address.
      _ => mem.ppu_addr_bus as u8,
    }
  }

  pub fn ppu_chr_read(&mut self, addr: u16) -> u8 {
    self.mapper.notify_chr_access(addr, &mut self.mem.banks);
    self.mem.chr[self.mem.banks.chr.translate(addr)]
  }

  pub fn ppu_vram_read(&self, addr: u16) -> u8 {
    self.mem.vram[self.mem.banks.vram.translate(addr)]
  }

  pub fn ppu_palette_read(&self, addr: u16) -> u8 {
    let addr = (addr as usize - 0x3f00) & 31;
    if addr % 4 == 0 {
      self.mem.palettes[0]
    } else {
      self.mem.palettes[addr]
    }
  }

  pub fn ppu_dispatch_write(&mut self, addr: u16, val: u8) {
    let mem = &mut self.mem;

    let addr = addr & 0x3fff;
    mem.ppu_addr_bus = addr;
    match addr {
      0x0000..=0x1fff => mem.chr[mem.banks.chr.translate(addr)] = val,
      0x2000..=0x3eff => mem.vram[mem.banks.vram.translate(addr & 0x2fff)] = val,
      0x3f00..=0x3fff => {
        let addr = (addr as usize - 0x3f00) & 31;
        let val = val & 0b11_1111;

        // if we're writing a transparent color
        if addr % 4 == 0 {
          // write both backdrop colors
          mem.palettes[addr & 0xf] = val;
          mem.palettes[addr & 0xf + 16] = val;
        } else {
          // write palette color as is
          mem.palettes[addr] = val;
        }
      }
      _ => {},
    }
  }
}