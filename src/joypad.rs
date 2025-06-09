use bitflags::bitflags;

bitflags! {
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[derive(Debug, Default, Clone, Copy)]
  pub struct JoypadButton: u8 {
  const Right  = 0b1000_0000;
  const Left   = 0b0100_0000;
  const Down   = 0b0010_0000;
  const Up   = 0b0001_0000;

  const Start  = 0b0000_1000;
  const Select = 0b0000_0100;
  const A    = 0b0000_0010;
  const B    = 0b0000_0001;
  }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct Joypad {
  strobe: bool,
  pub buttons1: JoypadButton,
  pub buttons2: JoypadButton,
  button_idx1: u8,
  button_idx2: u8,
}

// https://www.nesdev.org/wiki/Standard_controller
impl Joypad {
  pub fn write(&mut self, val: u8) {
    self.strobe = (val & 1) != 0;
    if self.strobe {
      self.button_idx1 = 0;
      self.button_idx2 = 0;
    }
  }

  pub fn read1(&mut self) -> u8 {
    if self.strobe {
      return self.buttons1.contains(JoypadButton::A) as u8;
    }

    let res = (self.buttons1.bits() >> self.button_idx1) & 1;
    self.button_idx1 = (self.button_idx1 + 1) % 8;
    // some games expect the highest bit to best due to open bus
    res | 0x40
  }

  pub fn read2(&mut self) -> u8 {
    if self.strobe {
      return self.buttons2.contains(JoypadButton::A) as u8;
    }

    let res = (self.buttons2.bits() >> self.button_idx2) & 1;
    self.button_idx2 = (self.button_idx2 + 1) % 8;
    // some games expect the highest bit to best due to open bus
    res | 0x40
  }
}
