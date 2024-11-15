use std::{collections::HashMap, path::Path};

use bus::Bus;
use cart::Cart;
use cpu::Cpu;
use joypad::JoypadStat;
use render::FrameBuffer;
use sdl2::{controller::Button, event::Event, keyboard::Keycode};

pub mod sdl2ctx;
pub mod render;

pub mod cpu;
pub mod instr;

pub mod mem;
pub mod bus;
pub mod mapper;

pub mod ppu;
pub mod joypad;

pub mod cart;

pub struct EmuConfigs {
  keymap: HashMap<Keycode, JoypadStat>,
  padmap: HashMap<Button, JoypadStat>,
}
impl EmuConfigs {
  pub fn new() -> Self {
    let default_keymap = HashMap::from([
        (Keycode::Z, JoypadStat::A),
        (Keycode::X, JoypadStat::B),
        (Keycode::UP, JoypadStat::UP),
        (Keycode::DOWN, JoypadStat::DOWN),
        (Keycode::LEFT, JoypadStat::LEFT),
        (Keycode::RIGHT, JoypadStat::RIGHT),
        (Keycode::N, JoypadStat::SELECT),
        (Keycode::M, JoypadStat::START),
    ]);

    let default_padmap = HashMap::from([
      (Button::A, JoypadStat::A),
      (Button::X, JoypadStat::B),
      (Button::B, JoypadStat::START),
      (Button::Y, JoypadStat::SELECT),
      (Button::Back, JoypadStat::SELECT),
      (Button::Start, JoypadStat::START),
      (Button::DPadLeft, JoypadStat::LEFT),
      (Button::DPadRight, JoypadStat::RIGHT),
      (Button::DPadUp, JoypadStat::UP),
      (Button::DPadDown, JoypadStat::DOWN),
    ]);

    EmuConfigs { keymap: default_keymap, padmap: default_padmap }
  }
}


pub struct Emulator {
  pub cpu: Cpu<Bus>,
  pub paused: bool,
  pub configs: EmuConfigs
}

impl Emulator {
  pub fn new(cart: Cart) -> Self {
    Self {
      cpu: Cpu::new(cart),
      paused: false,
      configs: EmuConfigs::new()
    }
  }

  pub fn empty() -> Self {
    Self {
      cpu: Cpu::new(Cart::empty()),
      paused: true,
      configs: EmuConfigs::new(),
    }
  }

  pub fn from_rom_path(rom_path: &Path) -> Result<Self, String> {
    let cart = Cart::new(rom_path);
    match cart {
      Ok(cart) => Ok(Emulator::new(cart)),
      Err(msg) => Err(msg.to_string())
    }
  }

  pub fn step(&mut self) {
    let cycles_at_start = self.cpu.cycles;
    self.cpu.step();
    self.cpu.bus.step(self.cpu.cycles - cycles_at_start);
  }

  pub fn step_until_vblank(&mut self) {
    loop {
      if self.paused { break; }
      if self.cpu.bus.peek_vblank() { break; }
      self.step();
    }
  }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen.0
  }
  
  pub fn handle_input(&mut self, event: &Event) {
    match event {
      Event::KeyDown { keycode, .. } => {
        if let Some(keycode) = keycode {
          if let Some(button) = self.configs.keymap.get(keycode) {
            self.cpu.bus.joypad.button.insert(*button);
          }
        }
      }
      Event::KeyUp { keycode, .. } => {
        if let Some(keycode) = keycode {
          if let Some(button) = self.configs.keymap.get(keycode) {
            self.cpu.bus.joypad.button.remove(*button);
          }
        }
      }
      Event::ControllerButtonDown { button, .. } => {
        if let Some(button) = self.configs.padmap.get(button) {
          self.cpu.bus.joypad.button.insert(*button);
        }
      }
      Event::ControllerButtonUp { button, .. } => {
        if let Some(button) = self.configs.padmap.get(button) {
          self.cpu.bus.joypad.button.remove(*button);
        }
      }
      Event::ControllerAxisMotion { .. } => {}
      _ => {}
    }
  }
}