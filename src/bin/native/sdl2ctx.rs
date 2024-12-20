use std::{collections::HashMap, error::Error};
use nen_emulator::{emu::Emu, joypad::JoypadButton};
use sdl2::{controller::{Axis, Button, GameController}, event::Event, keyboard::Keycode, render::{Canvas, TextureCreator}, video::{Window, WindowContext}, AudioSubsystem, EventPump, GameControllerSubsystem, Sdl, VideoSubsystem};

#[allow(unused)]
pub struct Sdl2Context {
  pub ctx: Sdl,
  pub video_subsystem: VideoSubsystem,
  pub audio_subsystem: AudioSubsystem,
  pub canvas: Canvas<Window>,
  pub texture_creator: TextureCreator<WindowContext>,
  pub events: EventPump,
  pub controller_subsystem: GameControllerSubsystem,
  pub controllers: Vec<GameController>,
  pub keymaps: Keymaps,
}

impl Sdl2Context {
  pub fn new(name: &str, width: u32, height: u32) -> Result<Self, Box<dyn Error>> {
    let ctx = sdl2::init()?;
    let video_subsystem= ctx.video()?;
    let audio_subsystem = ctx.audio()?;
    let window = video_subsystem.window(name, width, height)
        .position_centered()
        .resizable()
        .build()?;
    let canvas = window
        .into_canvas()
        .accelerated()
        .build()?;
    
    let texture_creator = canvas.texture_creator();
    let controller_subsystem = ctx.game_controller()?;
    let controllers = Vec::new();
    
    let events = ctx.event_pump()?;
    let keymaps = Keymaps::new();

    Ok(
      Self { ctx, video_subsystem, audio_subsystem, canvas, events, texture_creator, controller_subsystem, controllers, keymaps }
    )
  }
}


enum InputAction {
  Game(JoypadButton), Pause, Reset
}
const AXIS_DEAD_ZONE: i16 = 10_000;

pub struct Keymaps {
  keymap: HashMap<Keycode, InputAction>,
  padmap: HashMap<Button, InputAction>,
}
impl Keymaps {
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

    Keymaps { keymap: default_keymap, padmap: default_padmap }
  }
}

pub fn handle_input(keys: &Keymaps, event: &Event, emu: &mut Emu) {
  let joypad = emu.get_joypad();

  match event {
    Event::KeyDown { keycode, .. } 
    | Event::KeyUp { keycode, .. } => {
      if let Some(keycode) = keycode {
        if let Some(action) = keys.keymap.get(keycode) {
          match (action, event) {
            (InputAction::Game(button), Event::KeyDown {..}) => joypad.buttons1.insert(*button),
            (InputAction::Game(button), Event::KeyUp {..}) => joypad.buttons1.remove(*button),
            (InputAction::Pause, Event::KeyDown {..}) => {
              emu.is_paused = !emu.is_paused;
            },
            (InputAction::Reset, Event::KeyDown {..}) => emu.reset(),
            _ => {}
          }
        }
      }
    }
    Event::ControllerButtonDown { button, .. } 
    | Event::ControllerButtonUp { button, .. }  => {
      if let Some(action) = keys.padmap.get(button) {
        match (action, event) {
          (InputAction::Game(button), Event::ControllerButtonDown {..}) => joypad.buttons1.insert(*button),
          (InputAction::Game(button), Event::ControllerButtonUp {..}) => joypad.buttons1.remove(*button),
          (InputAction::Pause, Event::ControllerButtonDown {..}) => {
            emu.is_paused = !emu.is_paused;
          }
          (InputAction::Reset, Event::ControllerButtonDown {..}) => emu.reset(),
          _ => {}
        }
      }
    }

    Event::ControllerAxisMotion { axis: Axis::LeftX, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::RIGHT); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::LEFT); }
      else {
        joypad.buttons1.remove(JoypadButton::LEFT);
        joypad.buttons1.remove(JoypadButton::RIGHT);
      }
    }
    Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::DOWN); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::UP); }
      else {
        joypad.buttons1.remove(JoypadButton::UP);
        joypad.buttons1.remove(JoypadButton::DOWN);
      }
    }
    _ => {}
  }
}
