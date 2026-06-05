use crate::{emu::NesEmulator, joypad::JoypadBtn};

mod apu;
mod bus;
pub mod cpu;
pub mod emu;
pub mod games_db;
mod mapper;
mod ppu;
pub mod rom;

pub mod utils {
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

    #[derive(Default, Debug)]
    pub struct RingBuffer<T> {
        pub data: Box<[T]>,
        head: usize,
        tail: usize,
    }
    impl<T: Default + Clone> RingBuffer<T> {
        pub fn new(size: usize) -> Self {
            Self {
                data: vec![T::default(); size].into_boxed_slice(),
                head: 0,
                tail: 0,
            }
        }
    }

    impl<T> RingBuffer<T> {
        pub fn push(&mut self, val: T) {
            self.data[self.tail] = val;
            self.tail = (self.tail + 1) % self.data.len();
        }

        pub fn pop(&mut self) -> &T {
            let res = &self.data[self.head];
            self.head = (self.head + 1) % self.data.len();
            res
        }

        pub fn capacity(&self) -> usize {
            self.data.len()
        }

        pub fn is_queued_all_contiguos(&self) -> bool {
            // tail is right of head, consecutive data
            self.tail >= self.head
        }

        pub fn queued(&self) -> usize {
            if self.is_queued_all_contiguos() {
                // tail is right of head, consecutive
                self.tail - self.head
            } else {
                // tail is left of head, not consecutive
                self.tail + self.queued_contiguos()
            }
        }

        pub fn queued_contiguos(&self) -> usize {
            self.data.len() - self.head
        }

        pub fn available_contiguos(&self) -> usize {
            self.data.len() - self.tail
        }

        pub fn available(&self) -> usize {
            if self.is_queued_all_contiguos() {
                // tail is right of head, consecutive
                self.available_contiguos() + self.head
            } else {
                // tail is left of head, not consecutive
                self.head - self.tail
            }
        }

        pub fn take_available_contiguos(&mut self, amount: usize) -> (&[T], Option<&[T]>) {
            let amount = amount.min(self.data.len());

            let right_amount = amount.min(self.queued_contiguos());
            let right = &self.data[self.head..self.head + right_amount];

            let left = if right_amount < amount {
                let left_amount = amount - right_amount;
                Some(&self.data[..left_amount])
            } else {
                None
            };

            self.head = (self.head + amount) % self.data.len();
            (right, left)
        }
    }
}

#[cfg(feature = "savestates")]
use serde_big_array::BigArray;

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq)]
pub struct NesPalette(
    #[cfg_attr(feature = "savestates", serde(with = "BigArray"))] pub [(u8, u8, u8); 64],
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
      #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
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
    #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
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

impl NesEmulator {
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
