use crate::{cart::{CartHeader, Mirroring}, mem::MemMapping};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone)]
pub struct PrgBanking;
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone)]
pub struct ChrBanking;
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone)]
pub struct SramBanking;
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone)]
pub struct VramBanking;
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone)]
pub struct Banking<T> {
  pub data_size: usize,
  pub bank_size: usize,
  pub banks_count: usize,

  bank_size_shift: usize,
  banks_count_shift: usize,
  
  pages_start: usize,
  pub bankings: Box<[usize]>,
  kind: std::marker::PhantomData<T>
}

// https://stackoverflow.com/questions/25787613/division-and-multiplication-by-power-of-2
impl<T> Banking<T> {
  pub fn new(rom_size: usize, pages_start: usize, page_size: usize, pages_count: usize) -> Self {
    let bankings = vec![0; pages_count].into_boxed_slice();
    let bank_size = page_size;
    let banks_count = rom_size / bank_size;
    let bank_size_shift = bank_size.checked_ilog2().unwrap_or_default() as usize;
    let banks_count_shift = banks_count.checked_ilog2().unwrap_or_default() as usize;
    Self { bankings, data_size: rom_size, pages_start, bank_size, bank_size_shift, banks_count, banks_count_shift, kind: std::marker::PhantomData::<T> }
  }

  pub fn set_page(&mut self, page: usize, bank: usize) {
    // some games might write bigger bank numbers than really avaible
    // let bank = bank % self.banks_count;
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
}

impl Banking<PrgBanking> {
  pub fn new_prg(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 32*1024 / pages_count;
    Self::new(header.prg_size, 0x8000, pages_size, pages_count)
  }
}

impl Banking<SramBanking> {
  pub fn new_sram(header: &CartHeader) -> Self {
    Self::new(header.sram_real_size(), 0x6000, 8*1024, 1)
  }
}

impl Banking<ChrBanking> {
  pub fn new_chr(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 8*1024 / pages_count;
    Self::new(header.chr_real_size(), 0, pages_size, pages_count)
  }
}

impl Banking<VramBanking> {
  pub fn new_vram(header: &CartHeader) -> Self {
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemConfig {
  pub prg:  Banking<PrgBanking>,
  pub chr:  Banking<ChrBanking>,
  pub sram: Banking<SramBanking>,
  pub vram: Banking<VramBanking>,

  // we can't serialize a collection of function pointers
  #[cfg_attr(feature = "serde", serde(skip))]
  pub mapping: MemMapping,
}
impl Default for MemConfig {
  fn default() -> Self {
    let header = &Default::default();
    let prg = Banking::new_prg(header, 1);
    let chr = Banking::new_chr(header, 1);
    let sram = Banking::new_sram(header);
    let vram = Banking::new_vram(header);
    Self {prg, chr, sram, vram, mapping: MemMapping::default() }
  }
}

impl MemConfig {
  pub fn new(header: &CartHeader) -> Self {
    let prg = Banking::new_prg(header, 1);
    let chr = Banking::new_chr(header, 1);
    let sram = Banking::new_sram(header);
    let vram = Banking::new_vram(header);
    Self {prg, chr, sram, vram,  mapping: MemMapping::default() }
  }
}