use std::collections::HashMap;
use nen_emulator::{emu::Emu, joypad::JoypadButton};
use sdl2::{audio::{AudioQueue, AudioSpecDesired}, controller::{Axis, Button, GameController}, event::Event, keyboard::Keycode, render::{Canvas, TextureCreator}, video::{Window, WindowContext}, AudioSubsystem, EventPump, GameControllerSubsystem, Sdl, TimerSubsystem, VideoSubsystem};

#[allow(unused)]
pub struct Sdl2Context {
  pub ctx: Sdl,
  pub timer: TimerSubsystem,
  pub video_subsystem: VideoSubsystem,
  pub audio_subsystem: AudioSubsystem,
  pub audio_queue: AudioQueue<f32>,
  pub canvas: Canvas<Window>,
  pub texture_creator: TextureCreator<WindowContext>,
  pub events: EventPump,
  pub controller_subsystem: GameControllerSubsystem,
  pub controllers: Vec<GameController>,
  pub keymaps: Keymaps,
}

// TODO: refactor to return an error
impl Sdl2Context {
  pub fn new(name: &str, width: u32, height: u32) -> Self {
    let ctx = sdl2::init().expect("Couldn't initialize SDL2");
    let timer = ctx.timer().expect("Couldn't initialize timer subsytem");
    let video_subsystem= ctx.video().expect("Couldn't initialize video subsystem");
    let audio_subsystem = ctx.audio().expect("Couldn't initialize audio subsystem");
    let window = video_subsystem.window(name, width, height)
        .position_centered()
        .resizable()
        .build().expect("Couldn't initialize window");
    let canvas = window
        .into_canvas()
        .accelerated() // .present_vsync()
        .build().expect("Couldn't initialize drawing canvas");
    let texture_creator = canvas.texture_creator();
    let controller_subsystem = ctx.game_controller().expect("Couldn't initialize controller subsytem");
    
    let controllers = Vec::new();
    // let controllers_avaible = controller_subsystem.num_joysticks().expect("Couldn't get number of joysticks avaible");

    // eprintln!("Found {} joypads", controllers_avaible);
    // for i in 0..controllers_avaible {
    //   if !controller_subsystem.is_game_controller(i) { continue; }
      
    //   match controller_subsystem.open(i) {
    //     Ok(controller) => {
    //       eprintln!("Found controller: {}", controller.name());
    //       controllers.push(controller);
    //     }
    //     Err(e) => eprintln!("Couldn't open controller {i}: {e}"),
    //   }
    // }

    // if controllers.is_empty() {
    //   eprintln!("No game controllers found");
    // }

    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(2),
        samples: None,
    };

    let audio_queue = audio_subsystem
        .open_queue::<f32, _>(None, &desired_spec).expect("Couldn't open audio queue");
    audio_queue.resume();
    
    let events = ctx.event_pump().expect("Couldn't get the event pump");
    let keymaps = Keymaps::new();
    Self { ctx, video_subsystem, audio_subsystem, audio_queue, canvas, events, texture_creator, timer, controller_subsystem, controllers, keymaps }
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
            (InputAction::Game(button), Event::KeyDown {..}) => joypad.buttons.insert(*button),
            (InputAction::Game(button), Event::KeyUp {..}) => joypad.buttons.remove(*button),
            (InputAction::Pause, Event::KeyDown {..}) => emu.is_paused = !emu.is_paused,
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
          (InputAction::Game(button), Event::ControllerButtonDown {..}) => joypad.buttons.insert(*button),
          (InputAction::Game(button), Event::ControllerButtonUp {..}) => joypad.buttons.remove(*button),
          (InputAction::Pause, Event::ControllerButtonDown {..}) => emu.is_paused = !emu.is_paused,
          (InputAction::Reset, Event::ControllerButtonDown {..}) => emu.reset(),
          _ => {}
        }
      }
    }

    Event::ControllerAxisMotion { axis: Axis::LeftX, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons.insert(JoypadButton::RIGHT); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons.insert(JoypadButton::LEFT); }
      else {
        joypad.buttons.remove(JoypadButton::LEFT);
        joypad.buttons.remove(JoypadButton::RIGHT);
      }
    }
    Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons.insert(JoypadButton::DOWN); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons.insert(JoypadButton::UP); }
      else {
        joypad.buttons.remove(JoypadButton::UP);
        joypad.buttons.remove(JoypadButton::DOWN);
      }
    }
    _ => {}
  }
}
