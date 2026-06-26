use nenemu_core::{self, emu::NesEmulator, joypad};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
struct NesEmulatorWasm {
    emu: NesEmulator,
    rom: Vec<u8>,
}

#[wasm_bindgen]
struct EmulatorSamples {
    left: *const f32,
    right: *const f32,
}

// TODO: use log.error to print errors
// TODO: load zip files too (core can expose a function for this)

#[wasm_bindgen]
impl NesEmulatorWasm {
    pub fn load_from_bytes(rom: &[u8]) -> Result<Self, String> {
        NesEmulator::load_rom_from_bytes(rom, None::<&[u8]>)
            .map(|emu| Self {
                emu,
                rom: rom.to_vec(),
            })
            .map_err(|e| e.to_string())
    }

    pub fn empty() -> Self {
        Self {
            emu: NesEmulator::empty(),
            rom: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.emu = NesEmulator::load_rom_from_bytes(&self.rom, None::<&[u8]>).unwrap();
    }

    pub fn step(&mut self) {
        self.emu.step();
    }

    pub fn step_until_frame_ready(&mut self) -> Result<(), String> {
        self.emu.step_until_frame_ready().map_err(|e| e.to_string())
    }

    pub fn get_raw_frame_rgba(&mut self) -> *const u8 {
        self.emu.get_video_rgba().as_ptr()
    }

    pub fn get_audio_queued(&self) -> u32 {
        self.emu.audio_queued() as u32
    }

    pub fn get_raw_samples_f32(&mut self, amount: u32) -> EmulatorSamples {
        let (right, left) = self.emu.get_audio_f32(amount as usize);
        EmulatorSamples {
            right: right.as_ptr(),
            left: if let Some(left) = left {
                left.as_ptr()
            } else {
                std::ptr::null()
            },
        }
    }

    pub fn get_fps(&self) -> f32 {
        self.emu.frame_rate()
    }

    pub fn button_pressed(&mut self, button: u8) {
        self.emu
            .set_button(joypad::InputBtn::from_bits_retain(button), true);
    }

    pub fn button_released(&mut self, button: u8) {
        self.emu
            .set_button(joypad::InputBtn::from_bits_retain(button), false);
    }

    pub fn save_sram(&self) -> Option<Vec<u8>> {
        self.emu.save_battery().map(|bytes| bytes.to_vec())
    }

    pub fn load_sram(&mut self, data: Vec<u8>) -> Result<(), String> {
        self.emu.load_battery(&data).map_err(|e| e.to_string())
    }
}
