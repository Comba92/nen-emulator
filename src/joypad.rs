use bitflags::bitflags;

bitflags! {
  #[derive(Clone, Copy)]
  pub struct JoypadButton: u8 {
    const RIGHT  = 0b1000_0000;
    const LEFT   = 0b0100_0000;
    const DOWN   = 0b0010_0000;
    const UP     = 0b0001_0000;

    const START  = 0b0000_1000;
    const SELECT = 0b0000_0100;
    const A      = 0b0000_0010;
    const B      = 0b0000_0001;
  }
}
pub struct Joypad {
	strobe: bool,
	pub buttons1: JoypadButton,
	pub buttons2: JoypadButton,
	button_idx1: usize,
	button_idx2: usize,
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
			return self.buttons1.contains(JoypadButton::A) as u8;
		}

		let res = (self.buttons1.bits() >> self.button_idx1) & 1;
		self.button_idx1 = (self.button_idx1 + 1) % 8;
		res
	}

	pub fn read2(&mut self) -> u8 {
		if self.strobe {
			return self.buttons2.contains(JoypadButton::A) as u8;
		}

		let res = (self.buttons2.bits() >> self.button_idx2) & 1;
		self.button_idx2 = (self.button_idx2 + 1) % 8;
		res
	}
}