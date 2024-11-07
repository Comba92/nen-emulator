use bitflags::bitflags;

enum Button {
    A, B, Select, Start, Up, Down, Left, Right
}

use Button::*;
static BUTTONS_ORDER: [Button; 8] = [
    A, B, Select, Start, Up, Down, Left, Right
];

bitflags! {
    pub struct JoypadStat: u8 {
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
    pub strobe: bool,
    pub button: JoypadStat,
    pub button_idx: usize,
}
impl Joypad {
    pub fn new() -> Self {
        Joypad {
            strobe: false, button_idx: 0,
            button: JoypadStat::empty()
        }
    }

    pub fn write(&mut self, val: u8) {
        self.strobe = (val & 1) != 0;
        if self.strobe {
            self.button_idx = 0;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.strobe { return 1; }
        
        let res = (self.button.bits() 
            >> self.button_idx) & 1; 

        self.button_idx = (self.button_idx + 1) % 8;
        res
    }
}