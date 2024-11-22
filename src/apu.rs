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
    const unused        = 0b0010_0000;
    const frame_irq     = 0b0100_0000;
    const dmc_irq       = 0b1000_0000;
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

trait Waveform {
  fn step(&mut self, frame_count: u8);
  fn is_enabled(&self) -> bool;
}

#[derive(Default, Clone, Copy)]
pub struct Pulse {
  duty: PulseDutyMode,
  envelope_mode: PulseEnvelopeMode,
  volume_mode: PulseVolumeMode,
  volume: u8,

  sweep_on: bool,
  sweep_period: u8,
  sweep_negate: bool,
  sweep_shift: u8,

  timer: u16,
  length_counter: u8,
}

const TRIANGLE_SEQUENCE: [f32; 32] = [
  15.0, 14.0, 13.0, 12.0, 11.0, 10.0,  9.0,  8.0,  7.0,  6.0,  5.0,  4.0,  3.0,  2.0,  1.0,  0.0,
  0.0,  1.0,  2.0,  3.0,  4.0,  5.0,  6.0,  7.0,  8.0,  9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0
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
  pub frame_queue: Vec<f32>,
}

impl Triangle {
  pub fn step(&mut self) {
    if self.timer_count == 0 {
      self.timer_count = self.timer + 1;
      let duty_val = TRIANGLE_SEQUENCE[self.duty_idx];
      self.frame_queue.push(0.00851 * duty_val);
      self.duty_idx = (self.duty_idx + 1) % 32;
    }

    self.timer_count = self.timer_count.wrapping_add_signed(-1);
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

pub struct Apu {
  pub pulses: [Pulse; 2],
  pub triangle: Triangle,
  pub flags: ApuFlags,

  pub frame_mode: FrameCounterMode,
  pub frame_irq_off: bool,
  pub frame_count: u8,
  pub irq_requested: Option<()>,

  pub cycles: usize,
}

impl Apu {
  pub fn new() -> Self {
    let apu = Apu {
      flags: ApuFlags::empty(),
      pulses: [Pulse::default(); 2],
      triangle: Triangle::default(),

      frame_mode: FrameCounterMode::Step4,
      frame_irq_off: false,
      frame_count: 0,
      irq_requested: None,

      cycles: 0,
    };

    apu
  }

  pub fn step(&mut self) {
    // step the channels timers
    self.triangle.step();
    // step sequencer

    self.cycles += 1;
  }

  pub fn reg_read(&mut self, addr: u16) -> u8 {
    match addr {
      0x4015 => {
        // TODO: not implemented fully
        let res = self.flags.bits();
        self.flags.remove(ApuFlags::frame_irq);
        res
      }
      _ => 0
    }
  }

  pub fn reg_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x4000 | 0x4004 => {
        let pulse = 0;
        self.pulses[pulse].duty = PulseDutyMode::from((val >> 6) & 11);
        self.pulses[pulse].envelope_mode = PulseEnvelopeMode::from((val >> 5) & 1);
        self.pulses[pulse].volume_mode = PulseVolumeMode::from((val >> 4) & 1);
        self.pulses[pulse].volume = val & 0b1111;
      }
      0x4001 | 0x4005 => {
        let pulse = 0;
        self.pulses[pulse].sweep_on = val >> 7 != 0;
        self.pulses[pulse].sweep_period = (val >> 4) & 0b111;
        self.pulses[pulse].sweep_negate = (val >> 3) & 1 != 0;
        self.pulses[pulse].sweep_shift = val & 0b111;
      }
      0x4002 | 0x4006 => {
        let pulse = 0;
        self.pulses[pulse].timer = self.pulses[pulse].timer & 0xFF00
          | val as u16;
      }
      0x4003 | 0x4007 => {
        let pulse = 0;
        self.pulses[pulse].length_counter = val >> 3;
        self.pulses[pulse].timer = self.pulses[pulse].timer & 0x00FF
          | ((val as u16 & 0b111) << 8);
        self.pulses[pulse].length_counter = 0;
        // restart envelope
        // reset phase of pulse
      }
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
        let new = ApuFlags::from_bits_retain(val & 0b1_1111);
        // TODO: handle DMC if bit is set or cleared
        self.flags = self.flags.clone().intersection(new);
        self.flags.remove(ApuFlags::dmc_irq);
      }
      0x4017 => {
        self.frame_mode = FrameCounterMode::from((val >> 6) & 1);
        self.frame_irq_off = (val >> 7) & 1 != 0;

        if self.frame_mode == FrameCounterMode::Step5 {
          // TODO: clock all units
        }
      }
    _ => {}
    }
  }
}