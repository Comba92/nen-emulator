#[derive(Default)]
pub struct CpuBanking {
  banks: [usize; 5],
  banks_count: u16,
}

impl CpuBanking {
  pub fn new(banks_count: u16) -> Self {
    Self {
      banks_count,
      ..Default::default()
    }
  }

  pub fn fix_last_bank_8kb(&mut self) {
    self.banks[4] = self.banks_count as usize - 1;
  }

  pub fn set_bank_8kb(&mut self, page: u8, bank: u8) {
    let bank = bank as u16 & (self.banks_count-1);
    self.banks[page as usize] = (bank as usize) << 13;
  }

  pub fn set_bank_16kb(&mut self, page: u8, bank: u8) {
    self.set_bank_8kb(page, bank & !1);
    self.set_bank_8kb(page + 1, bank | 1);
  }

  pub fn set_bank_32kb(&mut self, page: u8, bank: u8) {
    let bank = bank & !0x3;
    self.set_bank_8kb(page, bank);
    self.set_bank_8kb(page + 1, bank + 1);
    self.set_bank_8kb(page + 2, bank + 2);
    self.set_bank_8kb(page + 3, bank + 3);
  }

  pub fn translate(&self, addr: u16) -> usize {
    let page = (addr) >> 13;
  }
}

pub struct PpuBanking {
  banks: [usize; 12],
  banks_count: u16,
}