use crate::{bus::Bus, cart::{CartHeader, Mirroring}, mem::Memory, ppu::Ppu};

pub fn set_byte_hi(dst: u16, val: u8) -> u16 {
  (dst & 0x00FF) | ((val as u16) << 8)
}

pub fn set_byte_lo(dst: u16, val: u8) -> u16 {
  (dst & 0xFF00) | val as u16
}

#[derive(Debug, Default)]
pub struct PrgBanking;
#[derive(Debug, Default)]
pub struct ChrBanking;
#[derive(Debug, Default)]
pub struct SramBanking;
#[derive(Debug, Default)]
pub struct CiramBanking;
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Banking {
  pub data_size: usize,
  pub bank_size: usize,
  bank_size_shift: usize,
  pub banks_count: usize,
  banks_count_shift: usize,
  pub pages_start: usize,
  pub bankings: Box<[usize]>,
  // kind: marker::PhantomData<T>
}

// https://stackoverflow.com/questions/25787613/division-and-multiplication-by-power-of-2
// impl<T> Banking<T> {
impl Banking {
  pub fn new(rom_size: usize, pages_start: usize, page_size: usize, pages_count: usize) -> Self {
    let bankings = vec![0; pages_count].into_boxed_slice();
    let bank_size = page_size;
    let cfg_count = rom_size / bank_size;
    let bank_size_shift = if bank_size != 0 { bank_size.ilog2() as usize } else { 0 };
    let cfg_count_shift = if cfg_count != 0 { cfg_count.ilog2() as usize } else { 0 };
    // Self { bankings, data_size: rom_size, pages_start, bank_size, bank_size_shift, banks_count: cfg_count, banks_count_shift: cfg_count_shift, kind: PhantomData::<T> }
    Self { bankings, data_size: rom_size, pages_start, bank_size, bank_size_shift, banks_count: cfg_count, banks_count_shift: cfg_count_shift }
  }

  pub fn set_page(&mut self, page: usize, bank: usize) {
    // some games might write bigger bank numbers than really avaible
    // let bank = bank % self.cfg_count;
    let bank = bank & (self.banks_count-1);
    // i do not expect to write outside the slots array.
    // self.bankings[page] = bank * self.bank_size;
    self.bankings[page] = bank << self.bank_size_shift;
  }

  pub fn swap_pages(&mut self, left: usize, right: usize) {
    self.bankings.swap(left, right);
  }

  pub fn set_page_to_last_bank(&mut self, page: usize) {
    let last_bank = self.banks_count-1;
    self.set_page(page, last_bank);
  }

  pub fn page_to_bank_addr(&self, page: usize, addr: usize) -> usize {
    // i do not expect to write outside the slots array here either. 
    // the bus object should take responsibilty to always pass correct addresses in range.
    // self.bankings[page] + (addr % self.bank_size)
    self.bankings[page] + (addr & (self.bank_size-1))
  }

  pub fn translate(&self, addr: usize) -> usize {
    // let page = (addr - self.pages_start) / self.bank_size;
    let page = (addr - self.pages_start) >> self.bank_size_shift;
    self.page_to_bank_addr(page, addr)
  }
// }

// impl Banking<PrgBanking> {
  pub fn new_prg(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 32*1024 / pages_count;
    Self::new(header.prg_size, 0x8000, pages_size, pages_count)
  }
// }

// impl Banking<SramBanking> {
  pub fn new_sram(header: &CartHeader) -> Self {
    Self::new(header.sram_real_size(), 0x6000, 8*1024, 1)
  }
// }

// impl Banking<ChrBanking> {
  pub fn new_chr(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 8*1024 / pages_count;
    Self::new(header.chr_real_size(), 0, pages_size, pages_count)
  }
// }

// impl Banking<CiramBanking> {
  pub fn new_ciram(header: &CartHeader) -> Self {
    let mut res = Self::new(4*1024, 0x2000, 1024, 4);
    if header.mirroring != Mirroring::FourScreen {
      res.banks_count = 2;
    }

    res.update(header.mirroring);
    res
  }

  pub fn update(&mut self, mirroring: Mirroring) {
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
      Mirroring::SingleScreenA => for i in 0..4 {
        self.set_page(i, 0);
      }
      Mirroring::SingleScreenB => for i in 0..4 {
        self.set_page(i, 1);
      }
      Mirroring::FourScreen => for i in 0..4 {
        self.set_page(i, i);
      }
    }
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct MemConfig {
  // pub prg:  Banking<PrgBanking>,
  // pub chr:  Banking<ChrBanking>,
  // pub sram: Banking<SramBanking>,
  // pub ciram: Banking<CiramBanking>,
  pub prg:  Banking,
  pub chr:  Banking,
  pub sram: Banking,
  pub ciram: Banking,

  #[serde(skip)]
  pub mapping: MemMapping,
}
impl Default for MemConfig {
  fn default() -> Self {
    let header = Default::default();
    Self::new(&header)
  }
}

impl MemConfig {
  pub fn new(header: &CartHeader) -> Self {
    let prg = Banking::new_prg(header, 1);
    let chr = Banking::new_chr(header, 1);
    let sram = Banking::new_sram(header);
    let ciram = Banking::new_ciram(header);
    let mapping = MemMapping::default();
    Self {prg, chr, sram, ciram, mapping}
  }
}

pub struct MemMapping {
  pub cpu_reads: [fn(&mut Bus, u16) -> u8; 8],
  pub cpu_writes: [fn(&mut Bus, u16, u8); 8],
  pub ppu_reads: [fn(&Ppu, u16) -> u8; 16],
  pub ppu_writes: [fn(&mut Ppu, u16, u8); 16],
}

impl MemMapping {
  pub fn set_vram_handlers(&mut self, read: fn(&Ppu, u16) -> u8, write: fn(&mut Ppu, u16, u8)) {
    for i in 8..12 {
      self.ppu_reads[i]  = read;
      self.ppu_writes[i] = write;
    }
  }
}

impl Default for MemMapping {
  fn default() -> Self {
    let cpu_reads = [
      |bus: &mut Bus, addr: u16| bus.ram[addr as usize & 0x7FF],
      |bus: &mut Bus, addr: u16| bus.ppu.read_reg(addr & 0x2007),
      |bus: &mut Bus, addr: u16| {
        match addr {
          0x4000..=0x4013 => bus.apu.read_reg(addr),
          0x4016 => bus.joypad.read1(),
          0x4017 => bus.joypad.read2(),
          0x4020..=0x5FFF => bus.cart.as_mut().mapper.cart_read(addr as usize),
          _ => 0,
        }
      },
      Bus::sram_read,
      Bus::prg_read,
      Bus::prg_read,
      Bus::prg_read,
      Bus::prg_read,
    ];

    let cpu_writes = [
      |bus: &mut Bus, addr: u16, val: u8| bus.ram[addr as usize & 0x7FF] = val,
      |bus: &mut Bus, addr: u16, val: u8| bus.ppu.write_reg(addr & 0x2007, val),
      |bus: &mut Bus, addr: u16, val: u8| {
        match addr {
          0x4000..=0x4013 => bus.apu.write_reg(addr as u16, val),
          0x4017 => {
            bus.apu.write_reg(addr as u16, val);
            bus.joypad.write(val);
          }
          0x4016 => bus.joypad.write(val),
          0x4014 => {
            bus.oam_dma.init(val);
            bus.tick();
          }
          0x4015 => {
            bus.apu.write_reg(addr as u16, val);
            bus.tick();
          }
          0x4020..=0x5FFF => {
            let cart = bus.cart.as_mut();
            cart.mapper.cart_write(&mut cart.cfg, addr as usize, val);
          }
          _ => {}
        }
      },
      Bus::sram_write,
      Bus::prg_write,
      Bus::prg_write,
      Bus::prg_write,
      Bus::prg_write,
    ];


    let ppu_reads = [
      Ppu::chr_read,
      Ppu::chr_read,
      Ppu::chr_read,
      Ppu::chr_read,
      Ppu::chr_read,
      Ppu::chr_read,
      Ppu::chr_read,
      Ppu::chr_read,
      Ppu::ciram_read,
      Ppu::ciram_read,
      Ppu::ciram_read,
      Ppu::ciram_read,
      Ppu::palettes_read,
      Ppu::palettes_read,
      Ppu::palettes_read,
      Ppu::palettes_read,
    ];

    let ppu_writes = [
      Ppu::chr_write,
      Ppu::chr_write,
      Ppu::chr_write,
      Ppu::chr_write,
      Ppu::chr_write,
      Ppu::chr_write,
      Ppu::chr_write,
      Ppu::chr_write,
      Ppu::ciram_write,
      Ppu::ciram_write,
      Ppu::ciram_write,
      Ppu::ciram_write,
      Ppu::palettes_write,
      Ppu::palettes_write,
      Ppu::palettes_write,
      Ppu::palettes_write,
    ];

    Self { cpu_reads, cpu_writes, ppu_reads, ppu_writes }
  }
}