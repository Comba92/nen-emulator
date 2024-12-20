use super::{Channel, Timer};

const DMC_SEQUENCE: [u16; 16] = [
  428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106,  84,  72,  54
];

#[derive(Default)]
pub(super) struct Dmc {
  irq_enabled: bool,
  mode: bool,
  timer: Timer,
  rate: u8,
  direct: u8,
  address: u8,
  length: u8,
}

impl Dmc {
  pub fn write_ctrl(&mut self, val: u8) {
    self.irq_enabled = val & 0b1000_0000 != 0;
    self.mode = val & 0b0100_0000 != 0 ;
    self.rate = val & 0b1111;
  }

  pub fn write_count(&mut self, val: u8) {
    self.direct = val & 0b0111_1111;
  }

  pub fn write_addr(&mut self, val: u8) {
    self.address = val;
  }

  pub fn write_length(&mut self, val: u8) {
    self.length = val;
  }
}

impl Channel for Dmc {
  fn step_timer(&mut self) {
    // TODO
  }

  fn step_half(&mut self) {
      
  }

  fn step_quarter(&mut self) {
      
  }

  fn is_enabled(&self) -> bool {
    false
  }

  fn set_enabled(&mut self, _enabled: bool) {
    // TODO
  }

  fn get_sample(&self) -> u8 {
    // TODO
    0
  }
}