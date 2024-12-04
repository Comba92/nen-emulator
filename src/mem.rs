pub trait Memory {
  fn read(&mut self, addr: u16) -> u8;
  fn write(&mut self, addr: u16, val: u8);

  fn read16(&mut self, addr: u16) -> u16 {
    let low = self.read(addr);
    let high = self.read(addr.wrapping_add(1));
    u16::from_le_bytes([low, high])
  }

  fn tick(&mut self) {}
  fn poll_nmi(&mut self)   -> bool { false }
  fn poll_irq(&mut self)   -> bool { false }

  fn wrapping_read16(&mut self, addr: u16) -> u16 {
    if addr & 0x00FF == 0x00FF {
      let page = addr & 0xFF00;
      let low = self.read(page | 0xFF);
      let high = self.read(page | 0x00);
      u16::from_le_bytes([low, high])
    } else { self.read16(addr) }
  }

  fn write16(&mut self, addr: u16, val: u16) {
    let [low, high] = val.to_le_bytes();
    self.write(addr, low);
    self.write(addr.wrapping_add(1), high);
  }
  
  fn write_data(&mut self, start: u16, data: &[u8]) {
    for (i , byte) in data.iter().enumerate() {
      self.write(start.wrapping_add(i as u16), *byte);
    }
  }
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