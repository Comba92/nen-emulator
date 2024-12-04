use core::f32;

use bitflags::bitflags;
use dmc::Dmc;
use noise::Noise;
use pulse::Pulse;
use triangle::Triangle;

mod pulse;
mod triangle;
mod noise;
mod dmc;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum EnvelopeMode {
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
enum VolumeMode {
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
struct Timer {
  pub period: u16,
  count: u16,
}
impl Timer {
  pub fn set_period_low(&mut self, val: u8) {
    self.period = self.period & 0xFF00
      | val as u16;
  }

  pub fn set_period_high(&mut self, val: u8) {
    self.period = self.period & 0x00FF
    | ((val as u16 & 0b111) << 8);
  }

  pub fn step<F: FnOnce(&mut Self)>(&mut self, callback: F) {
    self.count -= 1;
    if self.count == 0 {
      self.count = self.period + 1;
      callback(self);
    }
  }
}

const LENGTH_TABLE: [u8; 32] = [
  10, 254, 20,  2, 40,  4, 80,  6, 160,  8, 60, 10, 14, 12, 26, 14,
  12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30
];

#[derive(Default)]
struct LengthCounter {
  count: u8,
  pub halted: bool,
  pub enabled: bool,
}

impl LengthCounter {
  pub fn reload(&mut self, val: u8) {
    if self.enabled {
      let length_idx = val as usize >> 3;
      self.count = LENGTH_TABLE[length_idx];
    }
  }

  pub fn step(&mut self) {
    if !self.halted && self.count > 0 {
      self.count -= 1;
    }
  }

  pub fn is_enabled(&self) -> bool {
    self.count != 0
  }

  pub fn disable(&mut self) {
    self.enabled = false;
    self.count = 0;
  }
}

#[derive(Default)]
struct Envelope {
  pub start: bool,
  pub volume_and_envelope: u8,
  envelope_count: u8,
  pub decay_count: u8,
  pub envelope_mode: EnvelopeMode,
  pub volume_mode: VolumeMode,
}
impl Envelope {
  pub fn set(&mut self, val: u8) {
    self.envelope_mode = EnvelopeMode::from((val >> 5) & 1);
    self.volume_mode = VolumeMode::from((val >> 4) & 1);
    self.volume_and_envelope = val & 0b1111;
  }

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
trait Channel {
  fn step_timer(&mut self);
  fn step_length(&mut self) {}
  fn step_envelope(&mut self) {}
  fn step_sweep(&mut self, _complement: bool) {}
  fn step_linear(&mut self) {}

  fn is_enabled(&self) -> bool;
  fn set_enabled(&mut self, enabled: bool);
  fn get_sample(&self) -> u8;
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

bitflags! {
  #[derive(Clone, Default)]
  struct ApuFlags: u8 {
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

pub struct Apu {
  pulse1: Pulse,
  pulse2: Pulse,
  triangle: Triangle,
  noise: Noise,
  dmc: Dmc,
  
  frame_write_delay: usize,
  frame_mode: FrameCounterMode,
  dmc_irq_enabled: bool,
  frame_irq_enabled: bool,
  interrupts_disabled: bool,

  pub frame_irq_requested: Option<()>,
  pub current_sample: Option<i16>,

  // pub low_pass_filter: LowPassFilter,
  // pub high_pass_filters: [HighPassFilter; 2],

  cycles: usize,
}

impl Apu {
  pub fn new() -> Self {
    let apu = Apu {
      pulse1: Pulse::default(),
      pulse2: Pulse::default(),
      triangle: Triangle::default(),
      noise: Noise::default(),
      dmc: Dmc::default(),

      frame_write_delay: 0,
      frame_mode: FrameCounterMode::Step4,
      dmc_irq_enabled: false,
      frame_irq_enabled: false,
      interrupts_disabled: false,
      
      current_sample: None,
      frame_irq_requested: None,

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

  fn step_quarter_frame(&mut self) {
    self.pulse1.step_envelope();
    self.pulse2.step_envelope();
    self.triangle.step_linear();
    self.noise.step_envelope();
  }

  fn step_half_frame(&mut self) {
    self.step_quarter_frame();
    self.pulse1.step_length();
    self.pulse2.step_length();
    self.triangle.step_length();
    self.noise.step_length();

    self.pulse1.step_sweep(false);
    self.pulse2.step_sweep(true);
  }

  fn step_frame(&mut self) {
    match (self.cycles/2, &self.frame_mode) {
      (3728 | 11185, _) => self.step_quarter_frame(),
      (7465, _) => self.step_half_frame(),
      (14914, FrameCounterMode::Step4) => {
        self.step_half_frame();
        self.cycles = 0;

        self.frame_irq_enabled = !self.interrupts_disabled;

        if !self.interrupts_disabled {
          self.frame_irq_requested = Some(())
        }
      }
      (18640, FrameCounterMode::Step5) => {
        self.step_half_frame();
        self.cycles = 0;
      }
      _ => {}
    }
  }

  fn mix_channels(&mut self) {
    let pulse1   = self.pulse1.get_sample();
    let pulse2   = self.pulse2.get_sample();
    let triangle = self.triangle.get_sample();
    let noise    = self.noise.get_sample();

    let pulse_out = 0.00752 * (pulse1 + pulse2) as f32;
    let tnd_out = 0.00494 * noise as f32 + 0.00851 * triangle as f32;

    let sum = pulse_out + tnd_out;

    // let mut filtered;
    // filtered = self.high_pass_filters[0].process(sum);
    // filtered = self.high_pass_filters[1].process(filtered);
    // filtered = self.low_pass_filter.process(filtered);

    let output = (sum * u16::MAX as f32).clamp(0.0, u16::MAX as f32) as i16;
    self.current_sample = Some(output);
  }

  pub fn read_reg(&mut self, addr: u16) -> u8 {
    match addr {
      0x4015 => {
        let mut flags = ApuFlags::empty();
        flags.set(ApuFlags::pulse1, self.pulse1.is_enabled());
        flags.set(ApuFlags::pulse2, self.pulse2.is_enabled());
        flags.set(ApuFlags::triangle, self.triangle.is_enabled());
        flags.set(ApuFlags::noise, self.noise.is_enabled());
        flags.set(ApuFlags::dmc, self.dmc.is_enabled());
        flags.set(ApuFlags::frame_irq, self.frame_irq_enabled);
        flags.set(ApuFlags::dmc_irq, self.dmc_irq_enabled);

        // TODO: should not be cleared if read at the same moment of a read
        self.frame_irq_enabled = false;

        flags.bits()
      }
      _ => 0
    }
  }

  pub fn write_reg(&mut self, addr: u16, val: u8) {
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

      0x4010 => self.dmc.write_ctrl(val),
      0x4011 => self.dmc.write_count(val),
      0x4012 => self.dmc.write_addr(val),
      0x4013 => self.dmc.write_length(val),

      0x4015 => {
        self.pulse1.set_enabled(val & 0b0001 != 0);
        self.pulse2.set_enabled(val & 0b0010 != 0);
        self.triangle.set_enabled(val & 0b0100 != 0);
        self.noise.set_enabled(val & 0b1000 != 0);
        self.dmc.set_enabled(val & 0b1_0000 != 0);

        self.dmc_irq_enabled = false;
      }
      0x4017 => {
        self.frame_mode = FrameCounterMode::from((val >> 7) & 1);
        self.interrupts_disabled = (val >> 6) & 1 != 0;
        if self.interrupts_disabled {
          self.frame_irq_enabled = false;
        }

        // the timer is reset after 3 or 4 cpu cycles
        // https://www.nesdev.org/wiki/APU_Frame_Counter
        self.frame_write_delay = if self.cycles % 2 == 1 { 3 } else { 4 };

        if self.frame_mode == FrameCounterMode::Step5 {
          self.step_half_frame();
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
//       let c = sample_rate / f32::consts::PI / cutoff;
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
//       let c = sample_rate / f32::consts::PI / cutoff;
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