#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum EnvelopeMode {
  #[default] OneShot, Loop
}
impl From<u8> for EnvelopeMode {
  fn from(value: u8) -> Self {
    match value {
      0 => EnvelopeMode::OneShot,
      _ => EnvelopeMode::Loop,
    }
  }
}

#[derive(Default, Clone, Copy, PartialEq)]
enum VolumeMode {
  #[default] Envelope, Constant
}
impl From<u8> for VolumeMode {
  fn from(value: u8) -> Self {
    match value {
      0 => VolumeMode::Envelope,
      _ => VolumeMode::Constant,
    }
  }
}

#[derive(Default)]
pub(super) struct Envelope {
  pub start: bool,
  pub volume_and_envelope: u8,
  envelope_count: u8,
  decay_count: u8,
  envelope_mode: EnvelopeMode,
  volume_mode: VolumeMode,
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