use blip_buf::BlipBuf;

use crate::{emu::{self, Emu}, utils::{byte_set_hi, byte_set_lo}};

#[derive(Default)]
struct DividerCounter {
  count: u16,
  pub period: u16,
}

impl DividerCounter {
  fn step<F: FnOnce()>(&mut self, callback: F){
    if self.count > 0 {
      self.count -= 1;
    } else {
      self.reload();
      callback();
    }
  }

  fn reload(&mut self) {
    self.count = self.period + 1;
  }
}

#[derive(Default)]
struct LengthCounter {
  count: u8,
  enabled: bool,
  halted: bool,
}

impl LengthCounter {
  const TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
  ];

  fn step(&mut self) {
    if !self.halted && self.count > 0 {
      self.count -= 1;
    }
  }

  fn load(&mut self, val: u8) {
    if self.enabled {
      self.count = Self::TABLE[val as usize >> 3];
    }
  }

  fn is_enabled(&self) -> bool {
    self.count > 0
  }

  fn enable(&mut self, cond: bool) {
    self.enabled = cond;
    self.count = if cond { self.count } else { 0 };
  }
}

#[derive(Default)]
struct Envelope {
  start: bool,
  looping: bool,
  use_volume: bool,
  decay: u8,
  div: DividerCounter,
}

impl Envelope {
  fn step(&mut self) {
    self.div.step(|| {
      if self.decay > 0 {
        self.decay -= 1;
      } else if self.looping {
        self.decay = 15;
      }
    });

    if self.start {
      self.start = false;
      self.decay = 15;
      self.div.reload();
      return;
    }
  }

  fn set(&mut self, val: u8) {
    self.looping    = val & 0x20 != 0;
    self.use_volume = val & 0x10 != 0;
    self.div.period = val as u16 & 0xf;
  }

  fn volume(&self) -> u8 {
    if self.use_volume {
      self.div.period as u8
    } else {
      self.decay
    }
  }
}

#[derive(Default)]
struct Sweep {
  div: DividerCounter,
  enabled: bool,
  negate: bool,
  reload: bool,
  shift: u8,
  target_period: u16,
}

impl Sweep {
  fn set(&mut self, val: u8) {
    self.enabled = val & 0x80 != 0;
    self.div.period = ((val as u16) >> 4) & 0b111;
    self.negate = val & 0x8 != 0;
    self.shift = val & 0b111;
    self.reload = true;
  }
}

// https://www.nesdev.org/wiki/APU_Pulse
#[derive(Default)]
struct Pulse {
  div: DividerCounter,
  len: LengthCounter,
  env: Envelope,
  sweep: Sweep,
  duty_seq: u8,
  duty_cycle: u8, 
}

impl Pulse {
  const DUTIES: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 0, 0]
  ];

  fn write_ctrl(&mut self, val: u8) {
    self.env.set(val);
    self.len.halted = val & 0x10 != 0;
    self.duty_cycle = val >> 6;
  }

  fn write_sweep(&mut self, val: u8) {
    self.sweep.set(val);
  }

  fn write_timer_lo(&mut self, val: u8) {
    self.div.period = byte_set_lo(self.div.period, val);
  }

  fn write_timer_hi(&mut self, val: u8) {
    self.div.period = byte_set_hi(self.div.period, val & 0b111);
    self.len.load(val);

    self.duty_seq = 0;
    self.env.start = true;
  }

  fn step_divider(&mut self) {
    self.div.step(|| {
      self.duty_seq = (self.duty_seq + 1) % Self::DUTIES[0].len() as u8;
    });
  }

  fn is_muted(&self) -> bool {
    // Thus to fully disable the sweep unit, a program must additionally turn on the Negate flag, such as by writing $08. This ensures that the target period is not greater than the current period and therefore not greater than $7FF. 
    self.div.period < 8 || (!self.sweep.negate && self.div.period > 0x7ff)
  }

  fn step_sweep(&mut self, complement: bool) {
    // https://www.nesdev.org/wiki/APU_Sweep#Updating_the_period

    let is_muted = self.is_muted();
    let sweep = &mut self.sweep;

    sweep.div.step(|| {
      if sweep.enabled && sweep.shift > 0 {
        let period = self.div.period;

        if !is_muted {
          let change_amt = period >> sweep.shift;
          if sweep.negate {
            sweep.target_period = period.saturating_sub(change_amt);
            sweep.target_period -= complement as u16;
          }
          else {
            sweep.target_period = period + change_amt;
          }
          self.div.period = sweep.target_period;
        }
      }
    });

    if sweep.reload {
      sweep.div.count = sweep.div.period + 1;
      sweep.reload = false;
    }
  }

  fn sample(&self) -> u8 {
    if self.len.count > 0 && !self.is_muted() {
      let seq = Self::DUTIES[self.duty_cycle as usize][self.duty_seq as usize];
      seq * self.env.volume()
    } else {
      0
    }
  }
}

// https://www.nesdev.org/wiki/APU_Triangle
#[derive(Default)]
struct Triangle {
  div: DividerCounter,
  len: LengthCounter,
  linear_count: u8,
  linear_reload: u8,
  linear_reload_flag: bool,
  sequence: u8,
}

impl Triangle {
  const TABLE: &[u8] = &[
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
  ];

  fn step_divider(&mut self) {
    self.div.step(|| {
      // The sequencer is clocked by the timer as long as both the linear counter and the length counter are nonzero.
      
      // TODO: filter out ultrasonic frequencies
      if self.len.count > 0 && self.linear_count > 0 {
        self.sequence = (self.sequence + 1) % Self::TABLE.len() as u8;
      }
    });
  }

  fn linear_step(&mut self) {
    if self.linear_reload_flag {
      self.linear_count = self.linear_reload;
    } else if self.linear_count > 0 {
      self.linear_count -= 1;
    }

    if !self.len.halted {
      self.linear_reload_flag = false;
    }
  }

  fn enable(&mut self, cond: bool) {
    self.len.enable(cond);
    self.linear_count = if cond { self.linear_count } else { 0 };
  }

  fn sample(&self) -> u8 {
    // At the expense of accuracy, these can be eliminated in an emulator e.g. by halting the triangle channel when an ultrasonic frequency is set (a timer value less than 2). 
    // Other games, e.g. Zombie Nation and Bullet-Proof Software's Tetris, "silence" the triangle channel by setting the timer to $7FF, which produces a deep rumble and quiet whine. 
    if 2 <= self.div.period && self.div.period < 0x7ff {
      Self::TABLE[self.sequence as usize]
    } else {
      0
    }
  }
}

#[derive(Default)]
struct Noise {
  div: DividerCounter,
  len: LengthCounter,
  env: Envelope,
  looping: bool,
  shift: u16,
}

impl Noise {
  const TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160,
    202, 254, 380, 508, 762, 1016, 2034, 4068
  ];

  fn new() -> Self {
    Self {
      // On power-up, the shift register is loaded with the value 1.
      shift: 1,
      ..Default::default()
    }
  }

  fn step_divider(&mut self) {
    self.div.step(|| {
      let bit = if self.looping { 6 } else { 1 };
      let feedback = (self.shift & 1) ^ ((self.shift >> bit) & 1);
      self.shift >>= 1;
      // Bit 14, the leftmost bit, is set to the feedback calculated earlier
      self.shift |= feedback << 14;
    });
  }

  fn sample(&self) -> u8 {
    if self.len.count > 0 {
      !(self.shift & 1 == 0) as u8 * self.env.volume()
    } else {
      0
    }
  }
}

#[derive(Default)]
struct Dmc {

}

#[repr(u8)]
#[derive(Default)]
enum FrameMode {
  #[default] Step4, Step5
}

pub struct AudioBuf(pub BlipBuf);
impl Default for AudioBuf {
  fn default() -> Self {
    // TODO: make sample rate configurable
    let mut blip = BlipBuf::new(48000);
    blip.set_rates(1789773.0, 48000.0);
    Self(blip)
  }
}

#[derive(Default)]
pub struct ApuRP2A {
  p0: Pulse,
  p1: Pulse,
  tri: Triangle,
  noise: Noise,

  frame_count: u16,
  frame_irq_disable: bool,
  frame_mode: FrameMode,

  prev_sample: f32,
  pub blip: AudioBuf,
  pub cycles: usize,
}

impl ApuRP2A {
  pub fn new() -> Self {
    Self {
      noise: Noise::new(),
      ..Default::default()
    }
  }
}

impl Emu {
  pub fn apu_reg_read(&mut self, addr: u16) -> u8 {
    let apu = &mut self.apu;
    match addr {
      0x4015 => {
        let mut res = 0;
        res |= ((apu.p0.len.count  > 0) as u8) << 0;
        res |= ((apu.p1.len.count  > 0) as u8) << 1;
        res |= ((apu.tri.len.count > 0) as u8) << 2;
        res |= ((apu.noise.len.count > 0) as u8) << 3;
        res |= (self.events.contains(emu::Events::APU_FRAME) as u8) << 6;
        // TODO: If an interrupt flag was set at the same m\oment of the read, it will read back as 1 but it will not be cleared.

        self.events.remove(emu::Events::APU_FRAME);
        res
      }
      _ => 0,
    }
  }

  pub fn apu_reg_write(&mut self, addr: u16, val: u8) {
    let apu = &mut self.apu;
    
    match addr {
      0x4000 => apu.p0.write_ctrl(val),
      0x4001 => apu.p0.write_sweep(val),
      0x4002 => apu.p0.write_timer_lo(val),
      0x4003 => apu.p0.write_timer_hi(val),

      0x4004 => apu.p1.write_ctrl(val),
      0x4005 => apu.p1.write_sweep(val),
      0x4006 => apu.p1.write_timer_lo(val),
      0x4007 => apu.p1.write_timer_hi(val),

      0x4008 => {
        apu.tri.len.halted = val & 0x80 != 0;
        apu.tri.linear_reload = val & 0x7f;
      }
      0x400a => apu.tri.div.period = byte_set_lo(apu.tri.div.period, val),
      0x400b => {
        apu.tri.len.load(val);
        apu.tri.div.period = byte_set_hi(apu.tri.div.period, val & 0x7);
        apu.tri.linear_reload_flag = true;
      }

      0x400c => {
        apu.noise.env.set(val);
        apu.noise.len.halted = val & 0x20 != 0;
      }
      0x400e => {
        apu.noise.looping = val & 0x80 != 0;
        apu.noise.div.period = Noise::TABLE[val as usize & 0xf];
      }
      0x400f => {
        apu.noise.len.load(val);
        apu.noise.env.start = true;
      }

      0x4015 => {
        apu.p0.len.enable ((val >> 0) & 1 == 1);
        apu.p1.len.enable ((val >> 1) & 1 == 1);
        apu.tri.enable((val >> 2) & 1 == 1);
        apu.noise.len.enable((val >> 3) & 1 == 1);
      }

      0x4017 => {
        apu.frame_mode = if val & 0x80 == 0 {
          FrameMode::Step4
        } else {
          FrameMode::Step5
        };

        if val & 0x80 == 1 {
          // Writing to $4017 with bit 7 set ($80) will immediately clock all of its controlled units at the beginning of the 5-step sequence; with bit 7 clear, only the sequence is reset without clocking any of its units. 
          self.frame_half_step();
        }

        self.apu.frame_irq_disable = val & 0x40 != 0;
        self.apu.frame_count = 0;
        // TODO: Writing to $4017 resets the frame counter and the quarter/half frame triggers happen simultaneously, but only on "odd" cycles (and only after the first "even" cycle after the write occurs) – thus, it happens either 2 or 3 cycles after the write (i.e. on the 2nd or 3rd cycle of the next instruction). After 2 or 3 clock cycles (depending on when the write is performed), the timer is reset. 
      }
        _ => {}
    }
  }

  pub fn apu_step(&mut self) {
    self.apu.tri.step_divider();
    self.frame_count_step();

    if self.apu.cycles % 2 == 1 {
      self.apu.p0.step_divider();
      self.apu.p1.step_divider();
      self.apu.noise.step_divider();
    }

    let pulse = 0.00752 * (self.apu.p0.sample() + self.apu.p1.sample()) as f32;
    let tnd = 
      0.00851 * self.apu.tri.sample() as f32
      + 0.00494 * self.apu.noise.sample() as f32;

    let sample = (pulse + tnd) * 80000.0;
    // let sample = 100.0 * (self.apu.p0.sample() + self.apu.p1.sample()) as f32;
    let delta = sample - self.apu.prev_sample;

    self.apu.blip.0.add_delta(self.apu.cycles as u32, delta as i32);
    self.apu.prev_sample = sample;

    self.apu.cycles += 1;
  }

  fn frame_quarter_step(&mut self) {
    self.apu.p0.env.step();
    self.apu.p1.env.step();
    self.apu.tri.linear_step();
    self.apu.noise.env.step();
  }

  fn frame_half_step(&mut self) {
    self.frame_quarter_step();

    self.apu.p0.len.step();
    self.apu.p1.len.step();

    self.apu.p0.step_sweep(true);
    self.apu.p1.step_sweep(false);

    self.apu.tri.len.step();
    self.apu.noise.len.step();
  }

  fn frame_count_step(&mut self) {
    // The sequencer is clocked on every other CPU cycle, so 2 CPU cycles = 1 APU cycle
    // Every step is counted for cycle*2
    
    let apu = &mut self.apu;
    match (apu.frame_count, &apu.frame_mode) {
      (3728 | 11185, _) => self.frame_quarter_step(),
      (7456, _) => self.frame_half_step(),
      (14914, FrameMode::Step4) => {
        self.events.set(emu::Events::APU_FRAME, !apu.frame_irq_disable);
        
        apu.frame_count = 0;
        self.frame_half_step();
      }
      (18640, FrameMode::Step5) => {
        apu.frame_count = 0;
        self.frame_half_step();
      }
      _ => {}
    }

    self.apu.frame_count += 1;
  }
}