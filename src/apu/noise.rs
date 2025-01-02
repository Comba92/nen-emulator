use crate::cart::ConsoleTiming;

use super::{envelope::Envelope, Channel, LengthCounter, Timer};

const NOISE_PERIOD_NTSC: [u16; 16] = [
  4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];
const NOISE_PERIOD_PAL: [u16; 16] = [
  4, 8, 14, 30, 60, 88, 118, 148, 188, 236, 354, 472, 708,  944, 1890, 3778
];

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Noise {
  timing: ConsoleTiming,
  envelope: Envelope,
  loop_enabled: bool,
  timer: Timer,
  shift_reg: u16,
  length: LengthCounter,
  envelope_enabled: bool,
}

impl Default for Noise {
    fn default() -> Self {
      Self { timing: Default::default(), envelope_enabled: false, envelope: Default::default(), loop_enabled: Default::default(), timer: Default::default(), shift_reg: 1, length: Default::default() }
    }
}

impl Noise {
  pub fn new(timing: ConsoleTiming) -> Self {
    let mut res = Self::default();
    res.timing = timing;
    res
  }

  pub fn set_ctrl(&mut self, val: u8) {
    self.length.halted = (val >> 5) & 1 != 0;
    self.envelope.set(val);
  }
  
  fn period_table(&self) -> &[u16] {
    match self.timing {
      ConsoleTiming::PAL => &NOISE_PERIOD_PAL,
      _ => &NOISE_PERIOD_NTSC
    }
  }

  pub fn set_noise(&mut self, val: u8) {
    self.loop_enabled = (val >> 7) & 1 != 0;
    self.timer.period = self.period_table()[val as usize & 0b1111];
  }
  
  pub fn set_length(&mut self, val: u8) {
    self.length.reload(val);
    self.envelope.start = true;
  }

  fn is_muted(&self) -> bool {
    (self.shift_reg & 1) == 1
  }
}
impl Channel for Noise {
    fn step_timer(&mut self) {
      self.timer.step(|_| {
        let feedback = (self.shift_reg & 1) ^ (match self.loop_enabled {
          false => (self.shift_reg >> 1) & 1,
          true => (self.shift_reg >> 6) & 1
        });
        self.shift_reg >>= 1;
        self.shift_reg |= feedback << 14
      });
    }

    fn step_half(&mut self) {
      self.envelope.step();
    }

    fn step_quarter(&mut self) {
      self.length.step();
    }

    fn is_enabled(&self) -> bool { self.length.is_enabled() }

    fn set_enabled(&mut self, enabled: bool) { 
      if enabled { self.length.enabled = true; }
      else { 
        self.length.disable();
        self.envelope_enabled = false;
      }
    }

    // TODO: something makes it too noisy
    fn get_sample(&self) -> u8 {
      if !self.is_muted() && self.is_enabled() {
        self.envelope.volume()
      } else { 0 }
    }
}