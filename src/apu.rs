use core::{f32, mem};

use bitflags::bitflags;
use dmc::Dmc;
use noise::Noise;
use pulse::Pulse;
use triangle::Triangle;

use crate::cart::ConsoleTiming;

mod envelope;

mod pulse;
mod triangle;
mod noise;
mod dmc;

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
    self.count = self.count.wrapping_sub(1);
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

pub trait Channel: Default {
  fn step_timer(&mut self);
  fn step_quarter(&mut self);
  fn step_half(&mut self);

  fn is_enabled(&self) -> bool;
  fn set_enabled(&mut self, enabled: bool);
  fn get_sample(&self) -> u8;
}

#[derive(PartialEq)]
enum FrameCounterMode {
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
  struct Flags: u8 {
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
  pub dmc: Dmc,
  
  frame_write_delay: usize,
  frame_mode: FrameCounterMode,
  step_frame: fn(&mut Apu),

  irq_disabled: bool,
  pub frame_irq_flag: Option<()>,

  pub samples: Vec<f32>,
  samples_per_second: f32,
  sample_cycles: f32,

  low_pass_filter: LowPassIIR,

  cycles: usize,
}

// pub fn sample_f32_to_i16(sample: f32) -> i16 {
//   (sample * u16::MAX as f32).clamp(0.0, u16::MAX as f32) as i16
// }

impl Apu {
  pub fn new(timing: ConsoleTiming) -> Self {
    let frame_step = match timing {
      ConsoleTiming::PAL => Self::step_frame_pal,
      _ => Self::step_frame_ntsc,
    };

    let samples_per_second = 
      timing.frame_cpu_cycles() / ((44100.0 / timing.fps()) as f32);
    
    Self {
      pulse1: Pulse::default(),
      pulse2: Pulse::default(),
      triangle: Triangle::default(),
      noise: Noise::new(timing),
      dmc: Dmc::new(timing),

      frame_write_delay: 0,
      frame_mode: FrameCounterMode::Step4,
      step_frame: frame_step,
      
      irq_disabled: false,
      frame_irq_flag: None,

      samples: Vec::new(),
      samples_per_second,
      sample_cycles: 0.0,

      low_pass_filter: LowPassIIR
        ::new(1789773.0, 0.45 * 44100.0),

      cycles: 0,
    }
  }

  pub fn reset(&mut self) {
    self.pulse1.set_enabled(false);
    self.pulse2.set_enabled(false);
    self.triangle.set_enabled(false);
    self.noise.set_enabled(false);
    self.dmc.set_enabled(false);

    self.cycles = 0;
    self.sample_cycles = 0.0;
  }

  pub fn get_samples(&mut self) -> Vec<f32> {
    mem::take(&mut self.samples)
  }

  pub fn step(&mut self) {
    // A frame lasts 29780.5 CPU cycles.
    // We have to output 44100 hertz of samples per second.
    // We have 60 frames per second.
    // Meaning for a single frame we need 44100 / 60 = 735 samples.
    // Then, we have to output a sample every 29780.5 / 735 = 40.5 cycles!

    // if self.sample_cycles >= 40.517687075 {
    //   let sample = self.mix_channels();
    //   self.current_sample = Some(sample);
    //   self.sample_cycles -= 40.517687075;
    // }
    // self.sample_cycles += 1.0;

    // OPT: this if is EXTREMELY costly
    let sample = self.mix_channels();
    self.low_pass_filter.consume(sample);

    if self.sample_cycles >= self.samples_per_second {
      let output = self.low_pass_filter.output();
      self.samples.push(output);
      self.sample_cycles -= self.samples_per_second;
    }
    self.sample_cycles += 1.0;
    
    self.dmc.step_timer();
    self.triangle.step_timer();
    if self.cycles % 2 == 1 {
      self.pulse1.step_timer();
      self.pulse2.step_timer();
      self.noise.step_timer();

      (self.step_frame)(self)
    }
    
    // TODO: i dont know if this is correct thing to do
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
    self.pulse1.step_quarter();
    self.pulse2.step_quarter();
    self.triangle.step_quarter();
    self.noise.step_quarter();
  }

  fn step_half_frame(&mut self) {
    self.step_quarter_frame();
    self.pulse1.step_half();
    self.pulse2.step_half();
    self.triangle.step_half();
    self.noise.step_half();

    self.pulse1.step_sweep(false);
    self.pulse2.step_sweep(true);
  }

  fn step_frame_ntsc(&mut self) {
    match (self.cycles/2, &self.frame_mode) {
      (3728 | 11185, _) => self.step_quarter_frame(),
      (7465, _) => self.step_half_frame(),
      (14914, FrameCounterMode::Step4) => {
        self.step_half_frame();
        self.cycles = 0;

        if !self.irq_disabled {
          self.frame_irq_flag = Some(());
        }
      }
      (18640, FrameCounterMode::Step5) => {
        self.step_half_frame();
        self.cycles = 0;
      }
      _ => {}
    }
  }

  fn step_frame_pal(&mut self) {
    match (self.cycles/2, &self.frame_mode) {
      (4156 | 12469, _) => self.step_quarter_frame(),
      (8313, _) => self.step_half_frame(),
      (16626, FrameCounterMode::Step4) => {
        self.step_half_frame();
        self.cycles = 0;

        if !self.irq_disabled {
          self.frame_irq_flag = Some(());
        }
      }
      (20783, FrameCounterMode::Step5) => {
        self.step_half_frame();
        self.cycles = 0;
      }
      _ => {}
    }
  }

  fn mix_channels(&mut self) -> f32 {
    let pulse1   = self.pulse1.get_sample();
    let pulse2   = self.pulse2.get_sample();
    let triangle = self.triangle.get_sample();
    let noise    = self.noise.get_sample();
    let dmc = self.dmc.get_sample();

    let pulse_out = 0.00752 * (pulse1 + pulse2) as f32;
    let tnd_out = 
      0.00851 * triangle as f32
      + 0.00494 * noise as f32
      + 0.00335 * dmc as f32;
      
      let sum = pulse_out + tnd_out;
      sum
  }

  pub fn read_reg(&mut self, addr: u16) -> u8 {
    match addr {
      0x4015 => {
        let mut flags = Flags::empty();
        flags.set(Flags::pulse1, self.pulse1.is_enabled());
        flags.set(Flags::pulse2, self.pulse2.is_enabled());
        flags.set(Flags::triangle, self.triangle.is_enabled());
        flags.set(Flags::noise, self.noise.is_enabled());
        flags.set(Flags::dmc, self.dmc.is_enabled());
        // TODO: bit 5 is open bus
        flags.set(Flags::frame_irq, self.frame_irq_flag.is_some());
        flags.set(Flags::dmc_irq, self.dmc.irq_flag.is_some());

        self.frame_irq_flag = None;

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
      0x4011 => self.dmc.write_level(val),
      0x4012 => self.dmc.write_addr(val),
      0x4013 => self.dmc.write_length(val),

      0x4015 => {
        self.pulse1.set_enabled(val & 0b0001 != 0);
        self.pulse2.set_enabled(val & 0b0010 != 0);
        self.triangle.set_enabled(val & 0b0100 != 0);
        self.noise.set_enabled(val & 0b1000 != 0);
        self.dmc.set_enabled(val & 0b1_0000 != 0);

        self.dmc.irq_flag = None;
      }
      0x4017 => {
        self.frame_mode = FrameCounterMode::from((val >> 7) & 1);
        self.irq_disabled = (val >> 6) & 1 != 0;
        if self.irq_disabled {
          self.frame_irq_flag = None;
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

pub struct LowPassIIR {
  alpha: f32,
  previous_output: f32,
  delta: f32,
}

impl LowPassIIR {
  pub fn new(sample_rate: f32, cutoff_frequency: f32) -> LowPassIIR {
    let delta_t = 1.0 / sample_rate;
    let time_constant = 1.0 / (2.0 * f32::consts::PI * cutoff_frequency);
    let alpha = delta_t / (time_constant + delta_t);
    return LowPassIIR {
      alpha,
      previous_output: 0.0,
      delta: 0.0,
    }
  }

  pub fn consume(&mut self, new_input: f32) {
    self.previous_output = self.output();
    self.delta = new_input - self.previous_output;
  }

  pub fn output(&self) -> f32 {
    return self.previous_output + self.alpha * self.delta;
  }
}