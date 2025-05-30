pub trait Memory {
  fn read(&mut self, addr: u16) -> u8;
  fn write(&mut self, addr: u16, val: u8);

  fn tick(&mut self) {}

  fn nmi_poll(&mut self) -> bool { false }
  fn irq_poll(&mut self) -> bool { false }
}

pub struct Ram64Kb {
  pub mem: [u8; 64*1024]
}

impl Memory for Ram64Kb {
  fn read(&mut self, addr: u16) -> u8 {
    self.mem[addr as usize]
  }

  fn write(&mut self, addr: u16, val: u8) {
    self.mem[addr as usize] = val
  }
}