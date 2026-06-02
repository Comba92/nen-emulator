use crate::{emu::Emu, joypad::JoypadBtn};

mod apu;
mod blip;
mod bus;
pub mod cpu;
pub mod emu;
mod mapper;
mod ppu;
pub mod rom;

pub mod games_db;

mod utils {
    // pub fn bit_get(x: u8, bit: u8) -> bool { (x >> bit) & 1 == 1 }
    // pub fn bit_set(x: u8, flags: u8) -> u8 { x | flags }
    // pub fn bit_change(x: u8, flags: u8, cond: bool) -> u8 {
    //   if cond { bit_set(x, flags) }
    //   else    { bit_clear(x, flags) }
    // }
    // pub fn bit_clear(x: u8, flags: u8) -> u8 { x & !flags }
    // pub fn bit_toggle(x: u8, flags: u8) -> u8 { x ^ flags }

    pub fn byte_set_lo(x: u16, lo: u8) -> u16 {
        (x & 0xff00) | lo as u16
    }

    pub fn byte_set_hi(x: u16, hi: u8) -> u16 {
        use std::ops::Shl;
        (x & 0x00ff) | (hi as u16).shl(8)
    }
}

#[cfg(feature = "serde")]
use serde_big_array::BigArray;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq)]
pub struct NesPalette(
    #[cfg_attr(feature = "serde", serde(with = "BigArray"))] pub [(u8, u8, u8); 64],
);
impl Default for NesPalette {
    fn default() -> Self {
        Self([(0, 0, 0); 64])
    }
}

impl NesPalette {
    pub const MAX_SIZE: usize = 1536;

    // https://www.nesdev.org/wiki/.pal
    pub fn from_pal_file(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > Self::MAX_SIZE {
            return None;
        }

        let colors = bytes
            .chunks(3)
            // we take only the first palette set of 64 colors, more might be in a .pal file
            .take(64)
            .map(|rgb| (rgb[0], rgb[1], rgb[2]))
            .collect::<Vec<_>>();

        colors.try_into().ok().map(|x| Self(x))
    }
}

pub mod joypad {
    bitflags::bitflags! {
      #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
      #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
      pub struct JoypadBtn: u8 {
        const A = 1 << 0;
        const B = 1 << 1;
        const Select = 1 << 2;
        const Start = 1 << 3;
        const Up = 1 << 4;
        const Down = 1 << 5;
        const Left = 1 << 6;
        const Right = 1 << 7;
      }
    }

    #[derive(Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Joypad {
        polling: bool,
        curr_btn: u8,
        pub buttons: JoypadBtn,
    }

    impl Joypad {
        pub fn read(&mut self) -> u8 {
            if self.polling {
                let res = (self.buttons.bits() >> self.curr_btn) & 1;
                self.curr_btn = (self.curr_btn + 1) % 8;
                res
            } else {
                self.buttons.contains(JoypadBtn::A) as u8
            }
        }

        pub fn write(&mut self, val: u8) {
            self.polling = val & 1 == 0;
            self.curr_btn = if self.polling { 0 } else { self.curr_btn };
        }
    }
}

impl Emu {
    pub fn load_palette(&mut self, bytes: &[u8]) {
        if let Some(pal) = NesPalette::from_pal_file(bytes) {
            self.palette = pal;
        } else {
            eprintln!("not a valid palette file");
        }
    }

    pub fn set_button(&mut self, btn: JoypadBtn, state: bool) {
        self.joypad.buttons.set(btn, state);
    }
}
