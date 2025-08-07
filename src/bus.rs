use crate::{cart::Cart, emu::{self, Emu, Mirroring}, mapper::{Mapper, NROM}};

pub struct MemHandler {
  ram: [u8; 2 * 1024],
  prg: Vec<u8>,
  chr: Vec<u8>,
  pub vram: [u8; 2 * 1024],

  // consider keeping this in PPU
  pub palettes: [u8; 32],

  bankings: BankingHandler,
  mapper: Box<dyn Mapper>
}

pub struct BankingHandler {
  pub prg: Banking<()>,
  pub chr: Banking<()>,
  pub vram: Banking<()>,
}
impl Default for BankingHandler {
  fn default() -> Self {
    Self {
      prg: Banking::new(32 * 1024, 0x8000, 16 * 1024, 2),
      chr: Banking::new(8 * 1024, 0, 8 * 1024, 1),
      vram: Banking::new(2 * 1024, 0x2000, 1024, 4),
    }
  }
}

pub struct Banking<T> {
  data_size: usize,
  data_start: u16,
  bank_size: u16,
  bank_size_shift: usize,
  banks_count: usize,

  bankings: Vec<usize>,
  kind: std::marker::PhantomData<T>,
}

impl<T> Banking<T> {
  pub fn new(rom_size: usize, rom_start: u16, page_size: u16, pages_count: u16) -> Self {
    let bankings = vec![0; pages_count as usize];
    let bank_size = page_size;
    let banks_count = rom_size.checked_div(bank_size as usize).unwrap_or_default();
    // https://stackoverflow.com/questions/25787613/division-and-multiplication-by-power-of-2
    let bank_size_shift = bank_size.checked_ilog2().unwrap_or_default() as usize;

    Self {
      data_size: rom_size,
      data_start: rom_start,
      bank_size,
      bank_size_shift,
      banks_count,

      bankings,
      kind: std::marker::PhantomData::<T>,
    }
  }

  pub fn pages_count(&self) -> usize {
    self.bankings.len()
  }

  pub fn set_page(&mut self, page: usize, bank: usize) {
    // some games might write bigger bank numbers than really avaible
    // let bank = bank % self.banks_count;
    let bank = bank & (self.banks_count - 1);
    // i do not expect to write outside the slots array.
    // self.bankings[page] = bank * self.bank_size;
    self.bankings[page] = bank << self.bank_size_shift;
  }

  pub fn set_page_to_last_bank(&mut self, page: usize) {
    self.set_page(page, self.banks_count-1);
  }

  pub fn translate(&self, addr: u16) -> usize {
    // let page = (addr - self.pages_start) / self.bank_size;
    let page = (addr - self.data_start) >> self.bank_size_shift;
    // i do not expect to write outside the slots array here either.
    // the bus object should take responsibilty to always pass correct addresses in range.
    // self.bankings[page] + (addr % self.bank_size)
    self.bankings[page as usize] + (addr & (self.bank_size - 1)) as usize
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
        for i in 0..4 {
          self.set_page(i, i);
        }
      }
    }
  }
}

impl MemHandler {
  pub fn new(cart: Cart) -> Self {
    let mut banks = BankingHandler::default();
    banks.vram.mirror(&cart.header.mirroring);
    let mapper = NROM::new(&cart.header, &mut banks);

    Self {
      ram: [0; 2* 1024],
      prg: cart.prg,
      chr: cart.chr,
      vram: [0; 2 * 1024],
      palettes: [0; 32],
      
      // TODO: make banking based on mapper
      bankings: banks,
      mapper: mapper,
    }
  }
}

impl Emu {
  pub fn cpu_dispatch_read(&mut self, addr: u16) -> u8 {    
    let mem = &mut self.mem;
    match addr {
      0x0000..=0x1fff => mem.ram[addr as usize & 0x07ff],
      0x2000..=0x3fff => {
        self.ppu_reg_read(addr & 0x2007)
      }
      0x4016 => {
        0
      }
      0x8000..=0xffff => mem.prg[mem.bankings.prg.translate(addr)],
      // TODO: open bus
      _ => 0,
    }
  }

  pub fn cpu_dispatch_write(&mut self, addr: u16, val: u8) {    
    let mem = &mut self.mem;
    match addr {
      0x0000..=0x1fff => mem.ram[addr as usize & 0x07ff] = val,
      0x2000..=0x3fff => self.ppu_reg_write(addr & 0x2007, val),
      0x4014 => {
        // https://www.nesdev.org/wiki/PPU_registers#OAMDMA_-_Sprite_DMA_($4014_write)
        self.cpu_tick();
        // TODO: +1 cycle on odd cpu cyles

        let mut addr = (val as u16) << 8;

        // optimize this, as we always know we're writing to OAM
        for _ in 0..256 {
          let byte = self.cpu_dispatch_read(addr);
          addr += 1;
          self.cpu_tick();
          self.ppu_reg_write(0x2004, byte);
        }
      }
      0x4016 => {
        // TODO: joystick
      }

      0x8000..=0xffff => mem.prg[mem.bankings.prg.translate(addr)] = val,
      _ => {},
    }
  }

  pub fn ppu_dispatch_read(&mut self, addr: u16) -> u8 {
    let mem = &mut self.mem;
    match addr {
      0x0000..=0x1fff => mem.chr[mem.bankings.chr.translate(addr)],
      0x2000..=0x2fff => mem.vram[mem.bankings.vram.translate(addr)],
      0x3f00..=0x3fff => {
        let addr = (addr as usize - 0x3f00) & 31;
        if addr % 4 == 0 {
          mem.palettes[0]
        } else {
          mem.palettes[addr]
        }
      }
      // TODO: open bus
      _ => 0,
    }
  }

  pub fn ppu_dispatch_write(&mut self, addr: u16, val: u8) {
    let mem = &mut self.mem;
    match addr {
      0x0000..=0x1fff => mem.chr[mem.bankings.chr.translate(addr)] = val,
      0x2000..=0x2fff => mem.vram[mem.bankings.vram.translate(addr)] = val,
      0x3f00..=0x3fff => {
        let addr = (addr as usize - 0x3f00) & 31;
        let val = val & 0b11_1111;

        // if we're writing a transparent color
        if addr % 4 == 0 {
          // if addr == 0x3f00 || addr == 0x3f1f {
            // write all backdrop colors
            // for i in 0..8 {
            //   mem.palettes[i*4] = val;
            // }
            mem.palettes[addr & 0xf] = val;
            mem.palettes[addr & 0xf + 8] = val;
          // }
          // else ignore

        } else {
          // write palette color as is
          mem.palettes[addr] = val;
        }

        mem.palettes[addr] = val;

      }
      _ => {},
    }
  }
}