use std::{collections::HashMap, path::Path};

use bus::Bus;
use cart::Cart;
use cpu::Cpu;
use joypad::JoypadButton;
use render::FrameBuffer;
use sdl2::{controller::{Axis, Button}, event::Event, keyboard::Keycode};

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


enum InputAction {
  Game(JoypadButton), Pause, Reset
}
const AXIS_DEAD_ZONE: i16 = 10_000;

pub struct EmuConfigs {
  keymap: HashMap<Keycode, InputAction>,
  padmap: HashMap<Button, InputAction>,
}
impl EmuConfigs {
  pub fn new() -> Self {
    let default_keymap = HashMap::from([
      (Keycode::Z, InputAction::Game(JoypadButton::A)),
      (Keycode::X, InputAction::Game(JoypadButton::B)),
      (Keycode::UP, InputAction::Game(JoypadButton::UP)),
      (Keycode::DOWN, InputAction::Game(JoypadButton::DOWN)),
      (Keycode::LEFT, InputAction::Game(JoypadButton::LEFT)),
      (Keycode::RIGHT, InputAction::Game(JoypadButton::RIGHT)),
      (Keycode::N, InputAction::Game(JoypadButton::SELECT)),
      (Keycode::M, InputAction::Game(JoypadButton::START)),
      (Keycode::Space, InputAction::Pause),
      (Keycode::R, InputAction::Reset),
    ]);

    let default_padmap = HashMap::from([
      (Button::X, InputAction::Game(JoypadButton::A)),
      (Button::A, InputAction::Game(JoypadButton::B)),
      (Button::B, InputAction::Game(JoypadButton::START)),
      (Button::Y, InputAction::Game(JoypadButton::SELECT)),
      (Button::Back, InputAction::Game(JoypadButton::SELECT)),
      (Button::Start, InputAction::Game(JoypadButton::START)),
      (Button::DPadLeft, InputAction::Game(JoypadButton::LEFT)),
      (Button::DPadRight, InputAction::Game(JoypadButton::RIGHT)),
      (Button::DPadUp, InputAction::Game(JoypadButton::UP)),
      (Button::DPadDown, InputAction::Game(JoypadButton::DOWN)),
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

  pub fn reset(&mut self) {
    self.cpu.reset();
    self.cpu.bus.ppu.reset();
  }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen.0
  }
  
  pub fn handle_input(&mut self, event: &Event) {
    match event {
      Event::KeyDown { keycode, .. } => {
        if let Some(keycode) = keycode {
          if let Some(action) = self.configs.keymap.get(keycode) {
            match action {
              InputAction::Game(button) => self.cpu.bus.joypad.buttons.insert(*button),
              InputAction::Pause => self.paused = !self.paused,
              InputAction::Reset => self.reset(),
            }
          }
        }
      }
      Event::KeyUp { keycode, .. } => {
        if let Some(keycode) = keycode {
          if let Some(InputAction::Game(button)) = self.configs.keymap.get(keycode) {
            self.cpu.bus.joypad.buttons.remove(*button);
          }
        }
      }
      Event::ControllerButtonDown { button, .. } => {
        if let Some(action) = self.configs.padmap.get(button) {
          match action {
            InputAction::Game(action) => self.cpu.bus.joypad.buttons.insert(*action),
            InputAction::Pause => self.paused = !self.paused,
            InputAction::Reset => self.reset(),
          }
        }
      }
      Event::ControllerButtonUp { button, .. } => {
        if let Some(InputAction::Game(button)) = self.configs.padmap.get(button) {
          self.cpu.bus.joypad.buttons.remove(*button);
        }
      }
      Event::ControllerAxisMotion { axis: Axis::LeftX, value, .. } => {
        if *value > AXIS_DEAD_ZONE { self.cpu.bus.joypad.buttons.insert(JoypadButton::RIGHT); }
        else if *value < -AXIS_DEAD_ZONE { self.cpu.bus.joypad.buttons.insert(JoypadButton::LEFT); }
        else {
          self.cpu.bus.joypad.buttons.remove(JoypadButton::LEFT);
          self.cpu.bus.joypad.buttons.remove(JoypadButton::RIGHT);
        }
      }
      Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
        if *value > AXIS_DEAD_ZONE { self.cpu.bus.joypad.buttons.insert(JoypadButton::UP); }
        else if *value < -AXIS_DEAD_ZONE { self.cpu.bus.joypad.buttons.insert(JoypadButton::DOWN); }
        else {
          self.cpu.bus.joypad.buttons.remove(JoypadButton::UP);
          self.cpu.bus.joypad.buttons.remove(JoypadButton::DOWN);
        }
      }
      _ => {}
    }
  }
}
