pub trait Memory {
  fn read(&mut self, addr: u16) -> u8;
  fn write(&mut self, addr: u16, val: u8);

  fn read16(&mut self, addr: u16) -> u16 {
    let low = self.read(addr);
    let high = self.read(addr.wrapping_add(1));
    u16::from_le_bytes([low, high])
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