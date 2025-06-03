use super::{envelope::Envelope, Channel, LengthCounter, ApuDivider};

const PULSE_SEQUENCES: [[u8; 8]; 4] = [
  [ 0, 1, 0, 0, 0, 0, 0, 0 ],
  [ 0, 1, 1, 0, 0, 0, 0, 0 ],
  [ 0, 1, 1, 1, 1, 0, 0, 0 ],
  [ 1, 0, 0, 1, 1, 1, 1, 1 ],
  // [0,0,0,0,0,0,0,1],
  // [0,0,0,0,0,0,1,1],
  // [0,0,0,0,1,1,1,1],
  // [1,1,1,1,1,1,0,0],
];

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct Pulse {
  timer: ApuDivider,
  duty_mode: PulseDutyMode,
  duty_idx: usize,

  envelope: Envelope,
  
  sweep_enabled: bool,
  sweep_reload: bool,
  sweep_shift: u8,
  sweep_negate: bool,
  sweep_period: u8,
  sweep_count: u8,

  length: LengthCounter,
}
impl Pulse {
  pub fn set_ctrl(&mut self, val: u8) {
    self.duty_mode = PulseDutyMode::from((val >> 6) & 0b11);
    self.length.halted = (val >> 5) & 1 == 1;
    self.envelope.set(val);
  }

  pub fn set_sweep(&mut self, val: u8) {
    self.sweep_enabled = val >> 7 != 0;
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

  pub fn step_sweep(&mut self, complement: bool) {
    if self.sweep_count == 0 
      && self.sweep_enabled 
      && self.sweep_shift != 0
      && !self.is_muted() 
    {
      let change_amount = self.timer.period >> self.sweep_shift;
      if self.sweep_negate {
        self.timer.period = self.timer.period.saturating_sub(change_amount);
        if complement { 
          self.timer.period = self.timer.period.saturating_sub(1);
        }     
      } else {
        self.timer.period = self.timer.period + change_amount;
      }
    }

    if self.sweep_count == 0 || self.sweep_reload {
      self.sweep_count = self.sweep_period;
      self.sweep_reload = false;
    } else {
      self.sweep_count -= 1;
    }
  }

  fn is_muted(&self) -> bool {
    self.timer.period < 8 || (!self.sweep_negate && self.timer.period > 0x7FF)
  }
}

impl Channel for Pulse {
  fn step_timer(&mut self) {
    self.timer.step(|_| {
      self.duty_idx = 
        (self.duty_idx + 1) % PULSE_SEQUENCES[self.duty_mode as usize].len();
    });
  }

  fn step_quarter(&mut self) {
    self.envelope.step();
  }
  
  fn step_half(&mut self) {
    self.length.step();
  }

  fn is_enabled(&self) -> bool { self.length.is_enabled() }

  fn set_enabled(&mut self, enable: bool) { 
    if enable { self.length.enabled = true; }
    else { self.length.disable(); }
  }

  // TODO: something is wrong here
  fn get_sample(&self) -> u8 {
    let sample = PULSE_SEQUENCES[self.duty_mode as usize][self.duty_idx];
    if !self.is_muted() && self.is_enabled() { 
        sample * self.envelope.volume() 
    } else { 0 }
  }
}
