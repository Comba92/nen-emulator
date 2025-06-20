use super::{ApuDivider, Channel, LengthCounter};

const TRIANGLE_SEQUENCE: [u8; 32] = [
  15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
  13, 14, 15,
];

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub(super) struct Triangle {
  linear_reload: bool,
  linear_period: u8,
  linear_count: u8,
  length: LengthCounter,
  timer: ApuDivider,
  duty_idx: usize,
}

impl Triangle {
  pub fn set_ctrl(&mut self, val: u8) {
    self.length.halted = (val >> 7) != 0;
    self.linear_period = val & 0b0111_1111;
  }

  pub fn set_timer_low(&mut self, val: u8) {
    self.timer.period = self.timer.period & 0xFF00 | val as u16;
  }

  pub fn set_timer_high(&mut self, val: u8) {
    self.length.reload(val);
    self.timer.period = self.timer.period & 0x00FF | ((val as u16 & 0b111) << 8);
    self.linear_reload = true;
  }

  fn step_linear(&mut self) {
    if self.linear_reload {
      self.linear_count = self.linear_period;
    } else if self.linear_count > 0 {
      self.linear_count -= 1;
    }

    if !self.length.halted {
      self.linear_reload = false;
    }
  }
}
impl Channel for Triangle {
  fn step_timer(&mut self) {
    self.timer.step(|timer| {
      // timer period higher than 2 silences ultrasonic frequencies
      if self.length.count > 0
        && self.linear_count > 0
        && timer.period >= 2
        && timer.period < 0x7FE
      {
        self.duty_idx = (self.duty_idx + 1) % TRIANGLE_SEQUENCE.len();
      }
    });
  }

  fn step_quarter(&mut self) {
    self.step_linear();
  }

  fn step_half(&mut self) {
    self.length.step();
  }

  fn set_enabled(&mut self, enabled: bool) {
    if enabled {
      self.length.enabled = true;
    } else {
      self.length.disable();
      self.linear_count = 0;
    }
  }

  fn is_enabled(&self) -> bool {
    self.length.count > 0
  }

  fn get_sample(&self) -> u8 {
    TRIANGLE_SEQUENCE[self.duty_idx]
  }
}
