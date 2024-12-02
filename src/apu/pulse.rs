use std::ops::Neg;

use super::{Channel, Envelope, EnvelopeMode, LengthCounter, Timer, VolumeMode};

const PULSE_SEQUENCES: [[u8; 8]; 4] = [
  [ 0, 1, 0, 0, 0, 0, 0, 0 ],
  [ 0, 1, 1, 0, 0, 0, 0, 0 ],
  [ 0, 1, 1, 1, 1, 0, 0, 0 ],
  [ 1, 0, 0, 1, 1, 1, 1, 1 ]
];

#[derive(Default, Clone, Copy)]
enum PulseDutyMode {
  #[default] Duty12, Duty25, Duty50, Duty25Neg,
}
impl From<u8> for PulseDutyMode {
  fn from(value: u8) -> Self {
    match value {
      0 => PulseDutyMode::Duty12,
      1 => PulseDutyMode::Duty25,
      2 => PulseDutyMode::Duty50,
      3 => PulseDutyMode::Duty25Neg,
      _ => unreachable!("envelope mode is a value between 0 and 3 (included)")
    }
  }
}

#[derive(Default)]
pub struct Pulse {
  duty_mode: PulseDutyMode,
  envelope: Envelope,
  
  sweep_on: bool,
  sweep_reload: bool,
  sweep_shift: u8,
  sweep_negate: bool,
  sweep_period: u8,
  sweep_count: u8,
  
  timer: Timer,
  duty_idx: usize,

  length: LengthCounter,
}
impl Pulse {
  pub fn set_ctrl(&mut self, val: u8) {
    self.duty_mode = PulseDutyMode::from((val >> 6) & 11);
    self.length.halted = (val >> 5) & 1 == 1;
    self.envelope.envelope_mode = EnvelopeMode::from((val >> 5) & 1);
    self.envelope.volume_mode = VolumeMode::from((val >> 4) & 1);
    self.envelope.volume_and_envelope = val & 0b1111;
  }

  pub fn set_sweep(&mut self, val: u8) {
    self.sweep_on = val >> 7 != 0;
    self.sweep_period = (val >> 4) & 0b111;
    self.sweep_negate = (val >> 3) & 1 != 0;
    self.sweep_shift = val & 0b111;
    self.sweep_reload = true;
  }

  pub fn set_timer_low(&mut self, val: u8) { self.timer.set_period_low(val);}

  pub fn set_timer_high(&mut self, val: u8) {
    self.length.reload(val);
    self.timer.set_period_high(val);
    self.envelope.start = true;
    self.duty_idx = 0;
  }

  pub fn is_muted(&self) -> bool {
    self.timer.period < 8 || self.timer.period > 0x7FF
  }

  pub fn can_sample(&self) -> bool {
    self.length.count != 0 && !self.is_muted()
    && PULSE_SEQUENCES[self.duty_mode as usize][self.duty_idx] != 0
  }
}

impl Channel for Pulse {
  fn step_timer(&mut self) {
    self.timer.step(|_| {
      self.duty_idx = (self.duty_idx + 1) % PULSE_SEQUENCES[self.duty_mode as usize].len();
    });
  }

  fn step_length(&mut self) {
    self.length.step();
  }

  fn step_envelope(&mut self) {
    self.envelope.step();
  }

  // TODO: clean this shit up
  fn step_sweep(&mut self, complement: bool) {
    if self.sweep_reload {
      if self.sweep_on && self.sweep_count == 0 {
        let mut change_amount = (self.timer.period >> self.sweep_shift) as i16;
        if self.sweep_negate {
          change_amount = change_amount.neg();
          if !complement {
            change_amount = change_amount.wrapping_sub(1);
          }
        }
  
        let target_period = self.timer.period
          .checked_add_signed(change_amount)
          .unwrap_or(0);
  
        self.timer.period = target_period;
      }

      self.sweep_count = self.sweep_period + 1;
      self.sweep_reload = false;
    } else if self.sweep_count > 0 {
      self.sweep_count -= 1;
    } else {
      if self.sweep_on {
        let mut change_amount = (self.timer.period >> self.sweep_shift) as i16;
        if self.sweep_negate {
          change_amount = change_amount.neg();
          if !complement {
            change_amount = change_amount.wrapping_sub(1);
          }
        }
  
        let target_period = self.timer.period
          .checked_add_signed(change_amount)
          .unwrap_or(0);
  
        self.timer.period = target_period;
      }

      self.sweep_count = self.sweep_period + 1;
    }
  }

  fn is_enabled(&self) -> bool { self.length.is_enabled() }
  fn set_enabled(&mut self, enabled: bool) { 
    if enabled { self.length.enabled = true; }
    else { self.length.disable(); }
  }

  fn get_sample(&self) -> u8 {
    if self.can_sample() { self.envelope.volume() } else { 0 }
  }
}
