use super::{Channel, Envelope, LengthCounter, Timer};

const NOISE_SEQUENCE: [u16; 16] = [
  4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

pub struct Noise {
  envelope: Envelope,
  mode: bool,
  timer: Timer,
  shift_reg: u16,
  length: LengthCounter,
  envelope_on: bool,
}

impl Default for Noise {
    fn default() -> Self {
        Self { envelope_on: false, envelope: Default::default(), mode: Default::default(), timer: Default::default(), shift_reg: 1, length: Default::default() }
    }
}

impl Noise {
  pub fn set_ctrl(&mut self, val: u8) {
    self.length.halted = (val >> 5) & 1 != 0;
    self.envelope.set(val);
  }
  
  pub fn set_noise(&mut self, val: u8) {
    self.mode = (val >> 7) & 1 != 0;
    self.timer.period = NOISE_SEQUENCE[val as usize & 0b1111];
  }
  
  pub fn set_length(&mut self, val: u8) {
    self.length.reload(val);
    self.envelope.start = true;
  }
}
impl Channel for Noise {
    fn step_timer(&mut self) {
      self.timer.step(|_| {
        let feedback = (self.shift_reg & 1) ^ (match self.mode {
          false => (self.shift_reg >> 1) & 1,
          true => (self.shift_reg >> 6) & 1
        });
        self.shift_reg >>= 1;
        self.shift_reg |= feedback << 14 // | (self.shift_reg & 0x3FFF);
      });
    }

    fn step_envelope(&mut self) {
      self.envelope.step();
    }

    fn step_length(&mut self) {
      self.length.step();
    }

    fn is_enabled(&self) -> bool { self.length.is_enabled() }

    fn set_enabled(&mut self, enabled: bool) { 
      if enabled { self.length.enabled = true; }
      else { 
        self.length.disable();
        self.envelope_on = false;
      }
    }

    fn get_sample(&self) -> u8 {
      if (self.shift_reg & 1) != 1 && self.is_enabled() {
        self.envelope.volume()
      } else { 0 }
    }
}