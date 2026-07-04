use crate::{
    emu::{NesEmulator, SCREEN_HEIGHT, SCREEN_WIDTH},
    joypad::JoypadInput,
};

mod apu;
mod bus;
pub mod cpu;
pub mod emu;
pub mod games_db;
mod mapper;
mod ppu;
pub mod rom;

pub mod utils {
    use super::emu::*;

    pub fn bit_get(x: u8, bit: u8) -> bool {
        (x >> bit) & 1 == 1
    }
    pub fn bit_set(x: u8, flags: u8) -> u8 {
        x | flags
    }
    pub fn bit_change(x: u8, flags: u8, cond: bool) -> u8 {
        if cond {
            bit_set(x, flags)
        } else {
            bit_clear(x, flags)
        }
    }
    pub fn bit_clear(x: u8, flags: u8) -> u8 {
        x & !flags
    }
    pub fn bit_toggle(x: u8, flags: u8) -> u8 {
        x ^ flags
    }

    pub fn byte_set_lo(x: u16, lo: u8) -> u16 {
        (x & 0xff00) | lo as u16
    }

    pub fn byte_set_hi(x: u16, hi: u8) -> u16 {
        use std::ops::Shl;
        (x & 0x00ff) | (hi as u16).shl(8)
    }

    #[derive(Default, Debug)]
    pub struct RingBuffer<T> {
        pub(crate) data: Box<[T]>,
        read_pos: usize,
        write_pos: usize,
        queued: usize,
    }
    impl<T: Default + Clone> RingBuffer<T> {
        pub fn new(size: usize) -> Self {
            Self::new_with(size, Default::default())
        }
    }

    impl<T: Clone> RingBuffer<T> {
        pub fn new_with(size: usize, default: T) -> Self {
            Self {
                data: vec![default; size].into_boxed_slice(),
                read_pos: 0,
                write_pos: 0,
                queued: 0,
            }
        }
    }

    impl<T> RingBuffer<T> {
        pub fn clear(&mut self) {
            self.read_pos = 0;
            self.write_pos = 0;
        }

        pub fn read_pos(&self) -> usize {
            self.read_pos
        }

        pub fn write_pos(&self) -> usize {
            self.write_pos
        }

        pub fn push(&mut self, val: T) {
            self.data[self.write_pos] = val;
            self.write_pos = (self.write_pos + 1) % self.data.len();
            self.queued = (self.queued + 1).min(self.data.len());
        }

        pub fn pop(&mut self) -> &T {
            self.pop_mut()
        }

        pub fn pop_mut(&mut self) -> &mut T {
            let head = self.read_pos;
            self.read_pos = (self.read_pos + 1) % self.data.len();
            self.queued = self.queued.saturating_sub(1);
            let res = &mut self.data[head];
            res
        }

        pub fn capacity(&self) -> usize {
            self.data.len()
        }

        pub fn is_queued_all_contiguos(&self) -> bool {
            // tail is right of head, consecutive data
            self.write_pos >= self.read_pos
        }

        pub fn queued(&self) -> usize {
            if self.is_queued_all_contiguos() {
                // tail is right of head, consecutive
                self.write_pos - self.read_pos
            } else {
                // tail is left of head, not consecutive
                self.write_pos + self.queued_contiguos()
            }
            // self.queued
        }

        pub fn queued_contiguos(&self) -> usize {
            self.data.len() - self.read_pos
        }

        pub fn available_contiguos(&self) -> usize {
            self.data.len() - self.write_pos
        }

        pub fn available(&self) -> usize {
            self.data.len() - self.queued()
        }

        pub fn take(&mut self, amount: usize) -> (&[T], Option<&[T]>) {
            let amount = amount.min(self.queued());

            let right_amount = amount.min(self.queued_contiguos());
            let right = &self.data[self.read_pos..self.read_pos + right_amount];

            let left = if right_amount < amount {
                let left_amount = amount - right_amount;
                Some(&self.data[..left_amount])
            } else {
                None
            };

            self.read_pos = (self.read_pos + amount) % self.data.len();
            self.queued = self.queued.saturating_sub(amount);

            (right, left)
        }

        pub fn take_iter(&mut self, amount: usize) -> impl Iterator<Item = &T> {
            let (right, left) = self.take(amount);
            right.iter().chain(left.unwrap_or_default().iter())
        }
    }

    pub struct AvgResampler {
        sample_avg: f32,
        sample_count: usize,
        sample_timer: f32,
        cycles_per_sample: f32,
    }
    impl Default for AvgResampler {
        fn default() -> Self {
            Self::new(NTSC_CLOCK_RATE, SampleRate::default())
        }
    }

    impl AvgResampler {
        pub fn new(clock_rate: usize, frequency: SampleRate) -> Self {
            let freq: f32 = frequency.into();
            Self {
                sample_avg: 0.0,
                sample_count: 0,
                sample_timer: 0.0,
                cycles_per_sample: clock_rate as f32 / freq,
            }
        }

        pub fn clear(&self) -> Self {
            Self {
                sample_avg: 0.0,
                sample_count: 0,
                sample_timer: 0.0,
                cycles_per_sample: self.cycles_per_sample,
            }
        }

        pub fn set_rate(&mut self, clock_rate: usize, frequency: f32) {
            self.cycles_per_sample = clock_rate as f32 / frequency
        }

        pub fn add_sample(&mut self, sample: f32) -> Option<f32> {
            self.sample_avg += sample;
            self.sample_count += 1;
            self.sample_timer += 1.0;

            if self.sample_timer >= self.cycles_per_sample {
                self.sample_timer -= self.cycles_per_sample;
                let res = self.sample_avg / self.sample_count as f32;

                self.sample_avg = 0.0;
                self.sample_count = 0;

                Some(res)
            } else {
                None
            }
        }
    }
}

use bitflags::Flags;
#[cfg(feature = "savestates")]
use serde_big_array::BigArray;

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq)]
pub struct NesPalette(
    #[cfg_attr(feature = "savestates", serde(with = "BigArray"))] pub [(u8, u8, u8); 64],
);
impl Default for NesPalette {
    fn default() -> Self {
        Self::from_pal_file_bytes(include_bytes!("../utils/2C02G_wiki.pal")).unwrap()
    }
}

impl NesPalette {
    pub const MAX_SIZE: usize = 1536;

    // https://www.nesdev.org/wiki/.pal
    pub fn from_pal_file_bytes(bytes: &[u8]) -> Option<Self> {
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
      pub struct JoypadInput: u8 {
        // Order for first 8 buttons is important as they will iterate during polling
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

    #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
    pub struct Joypad {
        pub(crate) polling_controller: bool,
        pub(crate) current_btn_polled: u8,
        pub(crate) player1: JoypadInput,
        pub(crate) player2: JoypadInput,
        pub(crate) zapper_trigger: bool,
        pub(crate) zapper_outside: bool,
        pub(crate) zapper_pos: (isize, isize),
    }

    impl Default for Joypad {
        fn default() -> Self {
            Self {
                polling_controller: false,
                current_btn_polled: 0,
                player1: Default::default(),
                player2: Default::default(),
                zapper_pos: (-1, -1),
                zapper_trigger: false,
                zapper_outside: false,
            }
        }
    }

    impl Joypad {
        pub fn write(&mut self, val: u8) {
            self.polling_controller = val & 1 == 0;
            if self.polling_controller {
                self.current_btn_polled = 0;
            }
        }
    }
}

impl NesEmulator {
    fn read(&mut self, player: JoypadInput) -> u8 {
        let joy = &mut self.joy;
        let controller_input = if joy.polling_controller {
            let controller_btn = (joy.player1.bits() >> joy.current_btn_polled) & 1;
            joy.current_btn_polled = (joy.current_btn_polled + 1) % 8;

            controller_btn as u8
        } else {
            player.contains(JoypadInput::A) as u8
        };

        if self.rom_info().supports_zapper() {
            let zap_trigger = self.joy.zapper_trigger as u8;
            let zap_light = !self.is_zapper_light_sensed() || self.joy.zapper_outside;

            (zap_trigger << 4) | ((zap_light as u8) << 3) | controller_input
        } else {
            controller_input
        }
    }

    pub fn read_joypad1(&mut self) -> u8 {
        self.read(self.joy.player1) | (self.mem.cpu_open_bus & 0xe0)
    }

    pub fn read_joypad2(&mut self) -> u8 {
        // TODO: some games seems to not work with this
        // self.read(self.joy.player2) | (self.mem.cpu_open_bus & 0xe0)
        self.mem.cpu_open_bus
    }

    fn is_zapper_light_sensed(&mut self) -> bool {
        let click_x = self.joy.zapper_pos.0 as isize;
        let click_y = self.joy.zapper_pos.1 as isize;

        if click_x < 0 || click_y < 0 || click_x >= SCREEN_WIDTH || click_y >= SCREEN_HEIGHT {
            return false;
        }

        let ppu_x = self.ppu.dot as isize - 1;
        let ppu_y = self.ppu.line as isize;

        const LIGHT_RADIUS: isize = 3;

        for y in -LIGHT_RADIUS..=LIGHT_RADIUS {
            for x in -LIGHT_RADIUS..=LIGHT_RADIUS {
                let target_y = click_y + y;
                let target_x = click_x + x;

                if target_x < 0
                    || target_y < 0
                    || target_x >= SCREEN_WIDTH
                    || target_y >= SCREEN_HEIGHT
                {
                    continue;
                }

                // same as (target_y * 256 + target_x) * 4
                let pixel_idx = ((target_y << 8) | target_x) << 2;
                // sum the rgb components
                let pixel_brightness = self.output.videobuf_back.0[pixel_idx as usize + 0] as u16
                    + self.output.videobuf_back.0[pixel_idx as usize + 1] as u16
                    + self.output.videobuf_back.0[pixel_idx as usize + 2] as u16;

                // light can only be detected in bright color, and can only detect if the position is earlier than ppu current rendering position
                if pixel_brightness >= 100
                    && ppu_y >= target_y
                    && ppu_y.abs_diff(target_y) <= 20 // Tests in the Zap Ruder test ROM show that the photodiode stays on for about 26 scanlines with pure white, 24 scanlines with light gray, or 19 lines with dark gray
                    && (ppu_y != target_y || ppu_x >= target_x)
                {
                    return true;
                }
            }
        }

        return false;
    }

    pub fn set_button(&mut self, btn: JoypadInput, state: bool) {
        self.joy.player1.set(btn, state);
    }

    pub fn get_buttons(&self) -> JoypadInput {
        self.joy.player1
    }

    pub fn set_buttons_all(&mut self, input: JoypadInput) {
        self.joy.player1 = input;
    }

    pub fn clear_buttons_all(&mut self) {
        self.joy.player1.clear();
        self.joy.player2.clear();
    }

    pub fn set_zapper_trigger(&mut self, state: bool) {
        // TODO: this is currently only set as player 2 zapper (works on NTSC)

        // The large capacitor (10µF) inside the Zapper when combined with the 10kΩ pullup inside the console means that it will take approximately 100ms to change to "released" after the trigger has been half-pulled
        // This means a click too short (for example only when the click is just pressed) will not count as a trigger pull
        self.joy.zapper_trigger = state;
    }

    pub fn set_zapper_outside(&mut self, state: bool) {
        self.joy.zapper_outside = state;
    }

    pub fn set_zapper_light(&mut self, x: isize, y: isize) {
        self.joy.zapper_pos = (x, y);
    }
}
