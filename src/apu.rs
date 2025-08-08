use crate::{emu::Emu, utils::{byte_set_hi, byte_set_lo}};

#[derive(Default)]
struct Pulse {

}

const LENGTH_TABLE: [u8; 32] = [
  10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
  192, 24, 72, 26, 16, 28, 32, 30,
];

#[derive(Default)]
struct Triangle {
  length_enabled: bool,
  length_ctrl: bool,
  linear_load: u8,
  length_load: u8,
  length_reload: bool,
  timer_load: u16,
  timer_count: u16,

  length_count: u8,
  linear_count: u8,
  sequence: u8,
}
impl Triangle {
  const SEQUENCE: &[u8] = &[
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
 ];

  fn step_timer(&mut self) {
    if self.timer_count > 0 {
      self.timer_count -= 1;
    } else {
      self.timer_count = self.timer_load;
      if self.length_count > 0 && self.linear_count > 0 {
        self.sequence = (self.sequence + 1) & Self::SEQUENCE.len() as u8;
      }
    }
  }

  fn step_linear(&mut self) {
    if self.length_reload {
      self.linear_count = self.linear_load;
    } else if self.linear_count > 0 {
      self.linear_count -= 1;
    }

    if !self.length_ctrl {
      self.length_reload = false;
    }
  }

  fn step_length(&mut self) {
    if !self.length_ctrl && self.length_count > 0 {
      self.length_count -= 1;
    }
  }

  fn set_enabled(&mut self, enable: bool) {
    if enable {
      self.length_enabled = true;
    } else {
      self.length_enabled = false;
      self.length_count = 0;
      self.linear_count = 0;
    }
  }

  fn is_enabled(&self) -> bool {
    self.length_count > 0
  }

  fn sample(&self) -> u8 {
    Self::SEQUENCE[self.sequence as usize]
  }
}

#[derive(Default)]
pub struct ApuRP2A {
  pulse0: Pulse,
  tri: Triangle,
}

impl ApuRP2A {
  pub fn apu_read(&mut self, addr: u16) -> u8 {
    match addr {
      _ => 0,
    }
  }

  pub fn apu_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x4008 => {
        self.tri.length_ctrl = val & 0x80 != 0;
        self.tri.linear_load = val & 0x7f;
      }
      0x400a => self.tri.timer_load = byte_set_lo(self.tri.timer_load, val),
      0x400b => {
        self.tri.length_count = LENGTH_TABLE[(val >> 3) as usize];
        self.tri.timer_load = byte_set_hi(self.tri.timer_load, val & 0x7);
        self.tri.length_reload = true;
      }
      _ => {}
    }
  }
}

impl Emu {
  pub fn apu_step(&mut self) {
    self.apu.tri.step_timer();

    
  }
}