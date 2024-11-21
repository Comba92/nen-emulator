use bitflags::bitflags;

bitflags! {
  #[derive(Clone)]
  pub struct ApuFlags: u8 {
    const pulse1    = 0b0000_0001;
    const pulse2    = 0b0000_0010;
    const triangle  = 0b0000_0100;
    const noise     = 0b0000_1000;
    const dmc       = 0b0001_0000;
    const unused       = 0b0010_0000;
    const frame_irq_on = 0b0100_0000;
    const dmc_irq_on   = 0b1000_0000;
  }
}

pub enum PulseDutyMode {
  Duty12, Duty25, Duty50, Duty25Neg,
}

pub enum PulseEnvelopeMode {
  OneShot, Infinite
}

pub enum PulseVolumeMode {
  Envelope, Constant
}

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
  pulse1: Pulse,
  flags: ApuFlags,

  frame_mode: FrameCounterMode,
  frame_irq_on: bool, 
}

impl Apu {
  pub fn new() -> Self {
    Apu {
      flags: ApuFlags::empty(),
    }
  }

  pub fn reg_read(&mut self, addr: u16) -> u8 {
    match addr {
      0x4015 => {
        // TODO: not implemented fully
        let res = self.flags.bits();
        self.flags.remove(ApuFlags::frame_irq_on);
        res
      }
      _ => 0
    }
  }

  pub fn reg_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x4000 => {
        self.pulse1.duty = (val >> 6) & 11;
        self.pulse1.envelope_mode = (val >> 5) & 1;
        self.pulse1.volume_mode = (val >> 4) & 1;
        self.pulse1.volume = val & 0b1111;
      }
      0x4001 => {
        self.pulse1.sweep_on = val >> 7 != 0;
        self.pulse1.sweep_period = (val >> 4) & 0b111;
        self.pulse1.sweep_negate = (val >> 3) & 1 != 0;
        self.pulse1.sweep_shift = val & 0b111;
      }
      0x4002 => {
        self.pulse1.timer = self.pulse1.timer & 0xFF00
          | val as u16;
      }
      0x4003 => {
        self.pulse1.length_counter = val >> 3;
        self.pulse1.timer = self.pulse1.timer & 0x00FF
          | ((val as u16) << 8);
      }
      0x4015 => {
        self.flags = ApuFlags::from_bits_retain(val);
        // TODO: handle DMC if bit is set or cleared
        // TODO: clear DMC irq flag
      }
      0x4017 => {
        self.frame_mode = FrameCounterMode::from((val >> 6) & 1);
        self.frame_irq_on = (val >> 7) & 1 != 0;
      }
    _ => {}
    }
  }
}