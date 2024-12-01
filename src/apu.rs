use std::{collections::VecDeque, f32::consts::PI, ops::Neg};

use bitflags::bitflags;

pub const CPU_CLOCK: usize = 1789773;

bitflags! {
  #[derive(Clone, Default)]
  pub struct ApuFlags: u8 {
    const pulse1    = 0b0000_0001;
    const pulse2    = 0b0000_0010;
    const triangle  = 0b0000_0100;
    const noise     = 0b0000_1000;
    const dmc       = 0b0001_0000;
    const unused      = 0b0010_0000;
    const frame_irq   = 0b0100_0000;
    const dmc_irq     = 0b1000_0000;
  }
}

#[derive(Default, Clone, Copy)]
pub enum PulseDutyMode {
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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeMode {
  #[default] OneShot, Loop
}
impl From<u8> for EnvelopeMode {
  fn from(value: u8) -> Self {
    match value {
      0 => EnvelopeMode::OneShot,
      1 => EnvelopeMode::Loop,
      _ => unreachable!("envelope mode is either 0 or 1")
    }
  }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum VolumeMode {
  #[default] Envelope, Constant
}
impl From<u8> for VolumeMode {
    fn from(value: u8) -> Self {
        match value {
          0 => VolumeMode::Envelope,
          1 => VolumeMode::Constant,
          _ => unreachable!("volume mode is either 0 or 1")
        }
    }
}

#[derive(Default)]
pub struct Timer {
  pub period: u16,
  count: u16,
}
impl Timer {
  pub fn new(reload: u16) -> Self {
    Self {period: reload, count: reload}
  }

  pub fn set_period_low(&mut self, val: u8) {
    self.period = self.period & 0xFF00
      | val as u16;
  }

  pub fn set_period_high(&mut self, val: u8) {
    self.period = self.period & 0x00FF
    | ((val as u16 & 0b111) << 8);
  }

  pub fn step<F: FnOnce(&mut Self)>(&mut self, callback: F) {
    if self.count > 0 {
      self.count -= 1;
    } else {
      self.count = self.period + 1;
      callback(self);
    }
  }
}

#[derive(Default)]
pub struct LengthCounter {
  pub count: u8,
  pub halted: bool,
}
impl LengthCounter {
  pub fn reload(&mut self, val: u8) {
    //if !self.halted {
      let length_idx = val as usize >> 3;
      self.count = LENGTH_TABLE[length_idx];
    // }
  }

  pub fn step(&mut self) {
    if !self.halted && self.count > 0 {
      self.count -= 1;
    }
  }

  pub fn is_enabled(&self) -> bool {
    !self.halted && self.count != 0
  }

  pub fn disable(&mut self) {
    self.halted = true;
    self.count = 0;
  }
}

#[derive(Default)]
pub struct Envelope {
  pub start: bool,
  pub volume_and_envelope: u8,
  envelope_count: u8,
  pub decay_count: u8,
  pub envelope_mode: EnvelopeMode,
  pub volume_mode: VolumeMode,
}
impl Envelope {
  pub fn step(&mut self) {
    if self.start {
      self.start = false;
      self.decay_count = 15;
      self.envelope_count = self.volume_and_envelope + 1;
    } else if self.envelope_count > 0 {
      self.envelope_count -= 1;
    } else {
      self.envelope_count = self.volume_and_envelope + 1;

      if self.decay_count > 0 {
        self.decay_count -= 1;
      } else if self.envelope_mode == EnvelopeMode::Loop {
        self.decay_count = 15;
      }
    }
  }

  pub fn volume(&self) -> u8 {
      match self.volume_mode {
        VolumeMode::Envelope => self.decay_count,
        VolumeMode::Constant => self.volume_and_envelope,
      }
  }
}

// TODO: consider merging envelope and sweep in one
// TODO: step_timer, is_enabled and disable always do the same thing, can it be made smarter?
trait Channel {
  fn step_timer(&mut self);
  fn step_length(&mut self) {}
  fn step_envelope(&mut self) {}
  fn step_sweep(&mut self, _complement: bool) {}
  fn step_linear(&mut self) {}

  fn is_enabled(&self) -> bool;
  fn disable(&mut self);
  fn get_sample(&self) -> u8;
}

const LENGTH_TABLE: [u8; 32] = [
  10, 254, 20,  2, 40,  4, 80,  6, 160,  8, 60, 10, 14, 12, 26, 14,
  12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30
];

const PULSE_SEQUENCES: [[u8; 8]; 4] = [
  [ 0, 1, 0, 0, 0, 0, 0, 0 ],
  [ 0, 1, 1, 0, 0, 0, 0, 0 ],
  [ 0, 1, 1, 1, 1, 0, 0, 0 ],
  [ 1, 0, 0, 1, 1, 1, 1, 1 ]
];

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
  fn disable(&mut self) { self.length.disable(); }

  fn get_sample(&self) -> u8 {
    if self.can_sample() { self.envelope.volume() } else { 0 }
  }
}

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

  fn disable(&mut self) {
    self.length_count = 0;
    self.count_off = true;
  }

  fn is_enabled(&self) -> bool {
    !self.count_off && self.length_count != 0 && self.length_count != 0
  }
}

const NOISE_SEQUENCE: [u16; 16] = [
  4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

pub struct Noise {
  envelope: Envelope,
  mode: bool,
  timer: Timer,
  // TODO: Should be init at 1
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
    self.envelope.envelope_mode = EnvelopeMode::from((val >> 4) & 1);
    self.envelope.volume_and_envelope = val & 0b1111;
    self.envelope_on = (val >> 4) & 1 != 0;
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
      self.timer.step(|timer| {
        timer.count = timer.period;

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

    fn disable(&mut self) { self.length.disable(); self.envelope_on = false; }

    fn get_sample(&self) -> u8 {
      if (self.shift_reg & 1) != 1 && self.length.count != 0
      && self.envelope.volume_mode == VolumeMode::Envelope
      && self.envelope_on {
        self.envelope.volume()
      } else { 0 }
    }
}


#[derive(PartialEq, Eq)]
pub enum FrameCounterMode {
  Step4, Step5
}
impl From<u8> for FrameCounterMode {
    fn from(value: u8) -> Self {
      match value {
        0 => FrameCounterMode::Step4,
        _ => FrameCounterMode::Step5
      }
    }
}
impl Into<u8> for FrameCounterMode {
    fn into(self) -> u8 {
      match self {
        Self::Step4 => 0,
        Self::Step5 => 1,
      }
    }
}
pub enum FrameCounter {
  Quarted, Half, Full
}

pub struct Apu {
  pub pulse1: Pulse,
  pub pulse2: Pulse,
  pub triangle: Triangle,
  pub noise: Noise,
  
  pub frame_write_delay: usize,
  pub frame_mode: FrameCounterMode,
  pub dmc_irq_on: bool,
  pub frame_irq_on: bool,
  pub interrupts_off: bool,
  pub audio_buf: Vec<i16>,

  pub irq_requested: Option<()>,
  pub current_sample: Option<i16>,

  // pub low_pass_filter: LowPassFilter,
  // pub high_pass_filters: [HighPassFilter; 2],

  pub cycles: usize,
}

impl Apu {
  pub fn new() -> Self {
    let apu = Apu {
      pulse1: Pulse::default(),
      pulse2: Pulse::default(),
      triangle: Triangle::default(),
      noise: Noise::default(),

      frame_write_delay: 0,
      frame_mode: FrameCounterMode::Step4,
      dmc_irq_on: false,
      frame_irq_on: false,
      interrupts_off: false,
      audio_buf: Vec::new(),
      
      current_sample: None,
      irq_requested: None,

      // low_pass_filter: LowPassFilter::new(44_100.0, 14_000.0),
      // high_pass_filters: [
      //   HighPassFilter::new(44_100.0, 90.0),
      //   HighPassFilter::new(44_100.0, 440.0),
      // ],

      cycles: 0,
    };

    apu
  }

  pub fn reset(&mut self) {
    // TODO: reset APU
  }

  pub fn step(&mut self) {
    // A frame lasts 29780 CPU cycles.
    // We have to output 44100 hertz of samples per second.
    // We have 60 frames per second.
    // Meaning for a single frame we need 44100 / 60 = 735 samples.
    // Then, we have to output a sample every 29780 / 735 = 40 cycles!
    if self.cycles % 40 == 0 {
      self.mix_channels();
    }
    
    self.triangle.step_timer();
    if self.cycles % 2 == 1 {
      self.pulse1.step_timer();
      self.pulse2.step_timer();
      self.noise.step_timer();
      self.step_frame();
    }
    
    if self.frame_write_delay > 0 {
      self.frame_write_delay -= 1;
      if self.frame_write_delay == 0 {
        self.cycles = 0;
      }
    }

    // cycles count should reset to zero on the next cycle (thus we do not increase it at that time)
    if self.cycles == 14914 
    && self.frame_mode == FrameCounterMode::Step4 { 
      self.cycles = 0; 
    } else if self.cycles == 18640 
    && self.frame_mode == FrameCounterMode::Step5 {
      self.cycles = 0;
    } else {
      self.cycles += 1;
    }
  }

  pub fn step_frame(&mut self) {
    match (self.cycles/2, &self.frame_mode) {
      (3728 | 11185, _) => {
        self.pulse1.step_envelope();
        self.pulse2.step_envelope();
        self.triangle.step_linear();
        self.noise.step_envelope();
      }
      (7465, _) => {
        self.pulse1.step_envelope();
        self.pulse2.step_envelope();
        self.pulse1.step_sweep(false);
        self.pulse2.step_sweep(true);
        self.pulse1.step_length();
        self.pulse2.step_length();
        self.triangle.step_linear();
        self.triangle.step_length();
        self.noise.step_envelope();
        self.noise.step_length();
      }
      (14914, FrameCounterMode::Step4) => {
        self.pulse1.step_envelope();
        self.pulse2.step_envelope();
        self.pulse1.step_sweep(false);
        self.pulse2.step_sweep(true);
        self.pulse1.step_length();
        self.pulse2.step_length();
        self.triangle.step_linear();
        self.triangle.step_length();
        self.noise.step_envelope();
        self.noise.step_length();

        self.frame_irq_on = !self.interrupts_off;

        if self.frame_irq_on {
          self.irq_requested = Some(())
        }
      }
      (18640, FrameCounterMode::Step5) => {
        self.pulse1.step_envelope();
        self.pulse2.step_envelope();
        self.pulse1.step_sweep(false);
        self.pulse2.step_sweep(true);
        self.pulse1.step_length();
        self.pulse2.step_length();
        self.triangle.step_linear();
        self.triangle.step_length();
        self.noise.step_envelope();
        self.noise.step_length();
        
        if !self.interrupts_off && self.frame_irq_on {
          self.irq_requested = Some(())
        }
      }
      _ => {}
    }
  }

  pub fn mix_channels(&mut self) {
    let pulse1   = self.pulse1.get_sample();
    let pulse2   = self.pulse2.get_sample();
    let triangle = self.triangle.get_sample();
    let noise    = self.noise.get_sample();

    let pulse_out = 0.00752 * (pulse1 + pulse2) as f32;
    let tnd_out = 0.00851 * triangle as f32; // + 0.00494 * noise as f32;

    let sum = pulse_out + tnd_out;
    // let mut filtered = self.high_pass_filters[0].process(sum);
    // filtered = self.high_pass_filters[1].process(filtered);
    // filtered = self.low_pass_filter.process(filtered);

    let output = (sum * u16::MAX as f32).clamp(0.0, u16::MAX as f32) as i16;
    // self.audio_buf.push(output as i16);
    self.current_sample = Some(output);
  }

  pub fn reg_read(&mut self, addr: u16) -> u8 {
    match addr {
      0x4015 => {
        let mut flags = ApuFlags::empty();
        flags.set(ApuFlags::pulse1, self.pulse1.is_enabled());
        flags.set(ApuFlags::pulse2, self.pulse2.is_enabled());
        flags.set(ApuFlags::triangle, self.triangle.is_enabled());
        flags.set(ApuFlags::noise, self.noise.is_enabled());
        flags.set(ApuFlags::frame_irq, self.frame_irq_on);
        flags.set(ApuFlags::dmc_irq, self.dmc_irq_on);

        // TODO: should not be cleared if read at the same moment of a read
        self.frame_irq_on = false;

        flags.bits()
      }
      _ => 0
    }
  }

  pub fn reg_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x4000 => self.pulse1.set_ctrl(val),
      0x4004 => self.pulse2.set_ctrl(val),

      0x4001 => self.pulse1.set_sweep(val),
      0x4005 => self.pulse2.set_sweep(val),

      0x4002 => self.pulse1.set_timer_low(val),
      0x4006 => self.pulse2.set_timer_low(val),

      0x4003 => self.pulse1.set_timer_high(val),
      0x4007 => self.pulse2.set_timer_high(val),

      0x4008 => self.triangle.set_ctrl(val),
      0x400A => self.triangle.set_timer_low(val),
      0x400B => self.triangle.set_timer_high(val),

      0x400C => self.noise.set_ctrl(val),
      0x400E => self.noise.set_noise(val),
      0x400F => self.noise.set_length(val),

      0x4015 => {
        if val & 0b0001 == 0 { self.pulse1.disable(); }
        if val & 0b0010 == 0 { self.pulse2.disable(); }
        if val & 0b0100 == 0 { self.triangle.disable(); }
        if val & 0b1000 == 0 { self.noise.disable(); }

        self.dmc_irq_on = false;
      }
      0x4017 => {
        self.frame_mode = FrameCounterMode::from((val >> 7) & 1);
        self.interrupts_off = (val >> 6) & 1 != 0;
        if self.interrupts_off {
          self.frame_irq_on = false;
        }

        // the timer is reset after 3 or 4 cpu cycles
        // https://www.nesdev.org/wiki/APU_Frame_Counter
        self.frame_write_delay = if self.cycles % 2 == 1 { 3 } else { 4 };

        if self.frame_mode == FrameCounterMode::Step5 {
          self.pulse1.step_envelope();
          self.pulse2.step_envelope();
          self.pulse1.step_sweep(false);
          self.pulse2.step_sweep(true);
          self.pulse1.step_length();
          self.pulse2.step_length();
          self.triangle.step_linear();
          self.triangle.step_length();
          self.noise.step_envelope();
          self.noise.step_length();
        }
      }
    _ => {}
    }
  }
}

// pub struct LowPassFilter {
//   b0: f32,
//   b1: f32,
//   a1: f32,
//   prev_x: f32,
//   prev_y: f32,
// }

// impl LowPassFilter {
//   pub fn new(sample_rate: f32, cutoff: f32) -> Self {
//       let c = sample_rate / PI / cutoff;
//       let a0i = 1.0 / (1.0 + c);

//       Self {
//           b0: a0i,
//           b1: a0i,
//           a1: (1.0 - c) * a0i,
//           prev_x: 0.0,
//           prev_y: 0.0,
//       }
//   }

//   fn process(&mut self, signal: f32) -> f32 {
//     let y = self.b0 * signal + self.b1 * self.prev_x - self.a1 * self.prev_y;
//     self.prev_y = y;
//     self.prev_x = signal;
//     y
//   }
// }

// pub struct HighPassFilter {
//   b0: f32,
//   b1: f32,
//   a1: f32,
//   prev_x: f32,
//   prev_y: f32,
// }

// impl HighPassFilter {
//   pub fn new(sample_rate: f32, cutoff: f32) -> Self {
//       let c = sample_rate / PI / cutoff;
//       let a0i = 1.0 / (1.0 + c);

//       Self {
//           b0: c * a0i,
//           b1: -c * a0i,
//           a1: (1.0 - c) * a0i,
//           prev_x: 0.0,
//           prev_y: 0.0,
//       }
//   }

//   fn process(&mut self, signal: f32) -> f32 {
//       let y = self.b0 * signal + self.b1 * self.prev_x - self.a1 * self.prev_y;
//       self.prev_y = y;
//       self.prev_x = signal;
//       y
//   }
// }