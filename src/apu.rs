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

#[derive(Default, Clone, Copy)]
pub enum PulseEnvelopeMode {
  #[default] OneShot, Infinite
}
impl From<u8> for PulseEnvelopeMode {
  fn from(value: u8) -> Self {
    match value {
      0 => PulseEnvelopeMode::OneShot,
      1 => PulseEnvelopeMode::Infinite,
      _ => unreachable!("envelope mode is either 0 or 1")
    }
  }
}

#[derive(Default, Clone, Copy)]
pub enum PulseVolumeMode {
  #[default] Envelope, Constant
}
impl From<u8> for PulseVolumeMode {
    fn from(value: u8) -> Self {
        match value {
          0 => PulseVolumeMode::Envelope,
          1 => PulseVolumeMode::Constant,
          _ => unreachable!("volume mode is either 0 or 1")
        }
    }
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

#[derive(Default, Clone, Copy)]
pub struct Pulse {
  duty: PulseDutyMode,
  length_count_off: bool,
  envelope_mode: PulseEnvelopeMode,
  volume_mode: PulseVolumeMode,
  volume: u8,

  sweep_on: bool,
  sweep_period: u8,
  sweep_negate: bool,
  sweep_shift: u8,

  timer: u16,
  timer_count: u16,
  duty_idx: usize,
  length_count: u8,
}
impl Pulse {
  pub fn set_ctrl(&mut self, val: u8) {
    self.duty = PulseDutyMode::from((val >> 6) & 11);
    self.length_count_off = (val >> 5) & 1 != 0;
    self.envelope_mode = PulseEnvelopeMode::from((val >> 5) & 1);
    self.volume_mode = PulseVolumeMode::from((val >> 4) & 1);
    self.volume = val & 0b1111;
  }

  pub fn set_sweep(&mut self, val: u8) {
    self.sweep_on = val >> 7 != 0;
    self.sweep_period = (val >> 4) & 0b111;
    self.sweep_negate = (val >> 3) & 1 != 0;
    self.sweep_shift = val & 0b111;
  }

  pub fn set_timer_low(&mut self, val: u8) {
    self.timer = self.timer & 0xFF00
    | val as u16;
  }

  pub fn set_timer_high(&mut self, val: u8) {
    if !self.length_count_off {
      let length_idx = val as usize >> 3;
      self.length_count = LENGTH_TABLE[length_idx];
    }

    self.timer = self.timer & 0x00FF
      | ((val as u16 & 0b111) << 8);

    self.duty_idx = 0;

    // TODO: reload length counter, restart envelope and phase
  }

  pub fn step_timer(&mut self) {
    if self.timer_count == 0 {
      self.timer_count = self.timer + 1;
      self.duty_idx = (self.duty_idx + 1) % 8;
    } else {
      self.timer_count = self.timer_count.wrapping_add_signed(-1);
    }
  }

  pub fn step_length(&mut self) {
    if !self.length_count_off && self.length_count > 0 {
      self.length_count = self.length_count.wrapping_add_signed(-1);
    }
  }

  pub fn volume(&self) -> u8 {
    // TODO
    self.volume
  } 

  pub fn disable(&mut self) {
    self.length_count = 0;
    self.length_count_off = true;
  }

  pub fn is_enabled(&self) -> bool {
    !self.length_count_off && self.length_count != 0
  }

  pub fn can_sample(&self) -> bool {
    self.length_count != 0 && self.timer >= 8
  }

  pub fn get_sample(&self) -> u8 {
    PULSE_SEQUENCES[self.duty as usize][self.duty_idx]
  }
}

const TRIANGLE_SEQUENCE: [u8; 32] = [
  15, 14, 13, 12, 11, 10,  9,  8,  7,  6,  5,  4,  3,  2,  1,  0,
  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
];

#[derive(Default)]
pub struct Triangle {
  pub count_ctrl: bool,
  pub counter_reload: u8,
  pub linear_count: u8,
  pub length_count: u8,
  pub timer: u16,
  pub timer_count: u16,

  pub duty_idx: usize,
}

impl Triangle {
  pub fn step(&mut self) -> Option<u8> {
    let mut res = None;
    
    if self.timer_count == 0 {
      self.timer_count = self.timer + 1;
      let duty_val = TRIANGLE_SEQUENCE[self.duty_idx];
      self.duty_idx = (self.duty_idx + 1) % 32;
      res = Some(duty_val);
    }

    self.timer_count = self.timer_count.wrapping_add_signed(-1);
    res
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
  
  pub frame_write_delay: usize,
  pub frame_mode: FrameCounterMode,
  pub dmc_irq_on: bool,
  pub frame_irq_on: bool,
  pub interrupts_off: bool,
  pub irq_requested: Option<()>,
  pub samples_queue: Vec<f32>,

  pub cycles: usize,
}

impl Apu {
  pub fn new() -> Self {
    let apu = Apu {
      pulse1: Pulse::default(),
      pulse2: Pulse::default(),
      triangle: Triangle::default(),

      frame_write_delay: 0,
      frame_mode: FrameCounterMode::Step4,
      dmc_irq_on: false,
      frame_irq_on: false,
      interrupts_off: false,
      irq_requested: None,

      samples_queue: Vec::new(),
      cycles: 0,
    };

    apu
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
    
    if self.cycles % 2 == 1 {
      self.pulse1.step_timer();
      self.pulse2.step_timer();
      self.step_frame();
    }
    
    if self.frame_write_delay > 0 {
      self.frame_write_delay = self.frame_write_delay.wrapping_add_signed(-1);
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
    match (self.cycles, &self.frame_mode) {
      (3728, _) => {
        // clock envelopes and linear counter
      }
      (7465, _) => {
        // clock envelopes and linear counter
        // clock sweeps
        self.pulse1.step_length();
        self.pulse2.step_length();
      }
      (11185, _) => {
        // clock envelopes and linear counter
      }
      (14914, FrameCounterMode::Step4) => {
        // clock envelopes and linear counter
        // clock sweeps
        self.pulse1.step_length();
        self.pulse2.step_length();
        self.frame_irq_on = true;

        if !self.interrupts_off {
          self.irq_requested = Some(())
        }
      }
      (18640, FrameCounterMode::Step5) => {
        // clock envelopes and linear counter
        // clock sweeps
        self.pulse1.step_length();
        self.pulse2.step_length();
        if !self.interrupts_off && self.frame_irq_on {
          self.irq_requested = Some(())
        }
      }
      _ => {}
    }
  }

  pub fn mix_channels(&mut self) {
    let pulse1 = if self.pulse1.can_sample() {
      self.pulse1.get_sample()
    } else { 0 };

    let pulse2 = if self.pulse2.can_sample() {
      self.pulse2.get_sample()
    } else { 0 };

    self.samples_queue.push(0.00752 * (pulse1 + pulse2) as f32);
  }

  pub fn reg_read(&mut self, addr: u16) -> u8 {
    match addr {
      0x4015 => {
        let mut flags = ApuFlags::empty();
        flags.set(ApuFlags::pulse1, self.pulse1.is_enabled());
        flags.set(ApuFlags::pulse2, self.pulse2.is_enabled());
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

      0x4008 => {
        self.triangle.count_ctrl = (val >> 7) != 0;
        self.triangle.linear_count = val & 0b0111_1111;
      }
      0x400A => {
        self.triangle.timer = self.triangle.timer & 0xFF00
          | val as u16;
      }
      0x400B => {
        self.triangle.length_count = val >> 3;
        self.triangle.timer = self.triangle.timer & 0x00FF
          | ((val as u16 & 0b111) << 8);
      }
      0x4015 => {
        if val & 0b01 == 0 { self.pulse1.disable(); }
        if val & 0b10 == 0 { self.pulse2.disable(); }

        self.dmc_irq_on = false;
      }
      0x4017 => {
        self.frame_mode = FrameCounterMode::from((val >> 7) & 1);
        self.interrupts_off = (val >> 7) & 1 != 0;

        // the timer is reset after 3 or 4 cpu cycles
        // https://www.nesdev.org/wiki/APU_Frame_Counter
        self.frame_write_delay = if self.cycles % 2 == 1 { 3 } else { 4 };

        if self.frame_mode == FrameCounterMode::Step5 {
          // TODO: step envelopes and sweeps and other channels
          self.pulse1.step_length();
          self.pulse2.step_length();
        }
      }
    _ => {}
    }
  }
}