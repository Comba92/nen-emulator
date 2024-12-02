use super::{Channel, Timer, LENGTH_TABLE};

const TRIANGLE_SEQUENCE: [u8; 32] = [
  15, 14, 13, 12, 11, 10,  9,  8,  7,  6,  5,  4,  3,  2,  1,  0,
  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
];


#[derive(Default)]
pub struct Triangle {
  pub count_off: bool,
  pub linear_reload: bool,
  pub linear_period: u8,
  pub linear_count: u8,
  pub length_count: u8,
  pub timer: Timer,

  pub duty_idx: usize,
}

impl Triangle {
  pub fn set_ctrl(&mut self, val: u8) {
    self.count_off = (val >> 7) != 0;
    self.linear_period = val & 0b0111_1111;
  }

  pub fn set_timer_low(&mut self, val: u8) {
    self.timer.period = self.timer.period & 0xFF00
    | val as u16;
  }

  pub fn set_timer_high(&mut self, val: u8) {
    let length_idx = val as usize >> 3;
    self.length_count = LENGTH_TABLE[length_idx];

    self.timer.period = self.timer.period & 0x00FF
    | ((val as u16 & 0b111) << 8);

    self.linear_reload = true;
  }
}
impl Channel for Triangle {
  fn step_timer(&mut self) {
    self.timer.step(|_| {
      if self.length_count > 0 && self.linear_count > 0 {
        self.duty_idx = (self.duty_idx + 1) % TRIANGLE_SEQUENCE.len();
      }
    }); 
 }

  fn step_linear(&mut self) {
    if self.linear_reload {
      self.linear_count = self.linear_period;
    } else if self.linear_count > 0 {
      self.linear_count -= 1;
    }

    if !self.count_off { self.linear_reload = false; }
  }

  fn step_length(&mut self) {
    if !self.count_off && self.length_count > 0 {
      self.length_count -= 1;
    }
  }

  fn get_sample(&self) -> u8 {
    let sample = TRIANGLE_SEQUENCE[self.duty_idx];
    if sample > 2 && self.is_enabled() { sample } else { 0 }
  }


  fn set_enabled(&mut self, enabled: bool) { 
    if enabled { self.count_off = false; }
    else { 
      self.length_count = 0;
      self.count_off = true; 
    }
  }

  fn is_enabled(&self) -> bool {
    !self.count_off && self.length_count != 0 && self.length_count != 0
  }
}
