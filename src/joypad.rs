use bitflags::bitflags;

bitflags! {
  #[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
  pub struct JoypadButton: u8 {
    const right  = 0b1000_0000;
    const left   = 0b0100_0000;
    const down   = 0b0010_0000;
    const up     = 0b0001_0000;

    const start  = 0b0000_1000;
    const select = 0b0000_0100;
    const a      = 0b0000_0010;
    const b      = 0b0000_0001;
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Joypad {
	strobe: bool,
	pub buttons1: JoypadButton,
	pub buttons2: JoypadButton,
	button_idx1: u8,
	button_idx2: u8,
}

impl Joypad {
	pub fn new() -> Self {
		Joypad {
			strobe: false,
			button_idx1: 0,
			button_idx2: 0,
			buttons1: JoypadButton::empty(),
			buttons2: JoypadButton::empty(),
		}
	}

	pub fn write(&mut self, val: u8) {
		self.strobe = (val & 1) != 0;
		if self.strobe {
			self.button_idx1 = 0;
			self.button_idx2 = 0;
		}
	}

	pub fn read1(&mut self) -> u8 {
		if self.strobe {
			return self.buttons1.contains(JoypadButton::a) as u8;
		}

		let res = (self.buttons1.bits() >> self.button_idx1) & 1;
		self.button_idx1 = (self.button_idx1 + 1) % 8;
		// some games expect the highest bit to best due to open bus
		res | 0x40
	}

	pub fn read2(&mut self) -> u8 {
		if self.strobe {
			return self.buttons2.contains(JoypadButton::a) as u8;
		}

		let res = (self.buttons2.bits() >> self.button_idx2) & 1;
		self.button_idx2 = (self.button_idx2 + 1) % 8;
		// some games expect the highest bit to best due to open bus
		res | 0x40
	}
}