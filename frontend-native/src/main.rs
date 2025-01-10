use std::{collections::HashMap, error::Error, fs, io::Read, path::{Path, PathBuf}, time::{Duration, Instant}};
use nen_emulator::{joypad::JoypadButton as NesJoypadButton, nes::Nes};
use sdl2::{audio::{AudioQueue, AudioSpecDesired, AudioStatus}, controller::{Axis, Button}, event::Event, keyboard::Keycode};

enum InputAction {
  Game(NesJoypadButton), Pause, Reset, Mute, Save, Load
}

const AXIS_DEAD_ZONE: i16 = 10_000;
pub struct Keymaps {
  keymap: HashMap<Keycode, InputAction>,
  padmap: HashMap<Button, InputAction>,
}
impl Keymaps {
  pub fn new() -> Self {
    let default_keymap = HashMap::from([
      (Keycode::A, InputAction::Game(NesJoypadButton::a)),
      (Keycode::S, InputAction::Game(NesJoypadButton::b)),
      (Keycode::UP, InputAction::Game(NesJoypadButton::up)),
      (Keycode::DOWN, InputAction::Game(NesJoypadButton::down)),
      (Keycode::LEFT, InputAction::Game(NesJoypadButton::left)),
      (Keycode::RIGHT, InputAction::Game(NesJoypadButton::right)),
      (Keycode::E, InputAction::Game(NesJoypadButton::select)),
      (Keycode::W, InputAction::Game(NesJoypadButton::start)),
      (Keycode::Space, InputAction::Pause),
      (Keycode::R, InputAction::Reset),
      (Keycode::M, InputAction::Mute)
    ]);

    let default_padmap = HashMap::from([
      (Button::X, InputAction::Game(NesJoypadButton::a)),
      (Button::A, InputAction::Game(NesJoypadButton::b)),
      (Button::B, InputAction::Game(NesJoypadButton::start)),
      (Button::Y, InputAction::Game(NesJoypadButton::select)),
      (Button::Back, InputAction::Game(NesJoypadButton::select)),
      (Button::Start, InputAction::Game(NesJoypadButton::start)),
      (Button::DPadLeft, InputAction::Game(NesJoypadButton::left)),
      (Button::DPadRight, InputAction::Game(NesJoypadButton::right)),
      (Button::DPadUp, InputAction::Game(NesJoypadButton::up)),
      (Button::DPadDown, InputAction::Game(NesJoypadButton::down)),
    ]);

    Keymaps { keymap: default_keymap, padmap: default_padmap }
  }
}

fn open_rom(path: &Path) -> Result<Nes, Box<dyn Error>> {
	let mut bytes = Vec::new();
	let file = fs::File::open(path)?;

	let _ = zip::read::ZipArchive::new(file)
		.and_then(|mut archive|
			// we only take the first file in the archive, might be done in a smarter way
			archive.by_index(0)
			.map(|mut f| f.read_to_end(&mut bytes))
		).or_else(|_|
      // it is a raw .nes file
			fs::File::open(path).map(|mut f| f.read_to_end(&mut bytes))
		)?;

	
  Nes::boot_from_bytes(&bytes)
    .map_err(|msg| msg.into())
}

fn handle_input(keys: &Keymaps, event: &Event, ctx: &mut EmuCtx) {
  let emu = &mut ctx.emu;
  let joypad = emu.get_joypad();

  match event {
    Event::KeyDown { keycode, .. } 
    | Event::KeyUp { keycode, .. } => {
      if let Some(keycode) = keycode {
        if let Some(action) = keys.keymap.get(&keycode) {
          match (action, event) {
            (InputAction::Game(button), Event::KeyDown {..}) => joypad.buttons1.insert(*button),
            (InputAction::Game(button), Event::KeyUp {..}) => joypad.buttons1.remove(*button),
            (InputAction::Pause, Event::KeyDown {..}) => {
              ctx.is_paused = !ctx.is_paused;
              match &ctx.audio.status() {
                AudioStatus::Playing => ctx.audio.pause(),
                _=> ctx.audio.resume(),
              }
            },
            (InputAction::Reset, Event::KeyDown {..}) => emu.reset(),
            (InputAction::Mute, Event::KeyDown {..}) => {
              match &ctx.audio.status() {
                AudioStatus::Playing => ctx.audio.pause(),
                _=> ctx.audio.resume(),
              }
            }
            _ => {}
          }
        }
      }
    }

    Event::ControllerButtonDown { button, .. } 
    | Event::ControllerButtonUp { button, .. }  => {
      if let Some(action) = keys.padmap.get(&button) {
        match (action, event) {
          (InputAction::Game(button), Event::ControllerButtonDown {..}) => joypad.buttons1.insert(*button),
          (InputAction::Game(button), Event::ControllerButtonUp {..}) => joypad.buttons1.remove(*button),
          (InputAction::Pause, Event::ControllerButtonDown {..}) => {
            ctx.is_paused = !ctx.is_paused;
            match &ctx.audio.status() {
              AudioStatus::Playing => ctx.audio.pause(),
              _=> ctx.audio.resume(),
            }
          }
          (InputAction::Reset, Event::ControllerButtonDown {..}) => emu.reset(),
          (InputAction::Mute, Event::KeyDown {..}) => {
            match &ctx.audio.status() {
              AudioStatus::Playing => ctx.audio.pause(),
              _=> ctx.audio.resume(),
            }
          }
          _ => {}
        }
      }
    }

    Event::ControllerAxisMotion { axis: Axis::LeftX, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons1.insert(NesJoypadButton::right); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons1.insert(NesJoypadButton::left); }
      else {
        joypad.buttons1.remove(NesJoypadButton::left);
        joypad.buttons1.remove(NesJoypadButton::right);
      }
    }
    Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons1.insert(NesJoypadButton::down); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons1.insert(NesJoypadButton::up); }
      else {
        joypad.buttons1.remove(NesJoypadButton::up);
        joypad.buttons1.remove(NesJoypadButton::down);
      }
    }
    _ => {}
  }
}

struct EmuCtx {
  emu: Nes,
  is_paused: bool,
  is_running: bool,
  audio: AudioQueue<f32>,
  ms_frame: Duration,
}

fn main() {
  const SCALE: f32 = 3.5;
  const WINDOW_WIDTH:  u32  = (SCALE * 32  as f32* 8.0) as u32;
  const WINDOW_HEIGHT: u32  = (SCALE * 30 as f32* 8.0) as u32;

  let sdl = sdl2::init().unwrap();
  let video= sdl.video().unwrap();
  let audio = sdl.audio().unwrap();
  let window = video.window("NEN Emulator", WINDOW_WIDTH, WINDOW_HEIGHT)
      .position_centered()
      .resizable()
      .build()
      .unwrap();
  let mut canvas = window
      .into_canvas()
      .accelerated()
      .build()
      .unwrap();

  // keep aspect ratio
  canvas.set_logical_size(32*8, 30*8).unwrap();

  let controller = sdl.game_controller().unwrap();
  let mut controllers = Vec::new();
    
  let mut events = sdl.event_pump().unwrap();
  let texture_creator = canvas.texture_creator();

  let keymaps = Keymaps::new();

  let emu = Nes::boot_empty();

  let mut texture = texture_creator.create_texture_target(
    sdl2::pixels::PixelFormatEnum::RGBA32, emu.get_screen().width as u32, emu.get_screen().height as u32
  ).unwrap();

  let desired_spec = AudioSpecDesired {
    freq: Some(44100),
    channels: Some(1),
    samples: None,
  };

  let audio_dev = audio
    .open_queue::<f32, _>(None, &desired_spec).unwrap();

  let mut ctx = EmuCtx {
    ms_frame: Duration::from_secs_f32(1.0 / emu.get_fps()),
    is_paused: true,
    is_running: false,
    audio: audio_dev,
    emu,
  };

  const SAMPLES_PER_FRAME: u32 = 735;
  
  'running: loop {
    let ms_since_start = Instant::now();

    if !ctx.is_paused {
      ctx.emu.step_until_vblank();

      let is_muted = ctx.audio.status() != AudioStatus::Playing;

      // if you don't have enough audio, we run for another frame
      if !is_muted && ctx.audio.size() < SAMPLES_PER_FRAME*3 {
        ctx.emu.step_until_vblank();
      }

      if is_muted { ctx.emu.get_samples(); }
      else {
        ctx.audio.queue_audio(&ctx.emu.get_samples()).unwrap();
      }
    }

    for event in events.poll_iter() {
      if ctx.is_running {
        handle_input(&keymaps, &event, &mut ctx);
      }

      match event {
        Event::Quit { .. } => {
          break 'running;
        }
        Event::DropFile { filename, .. } => {
          ctx.audio.pause();
          ctx.audio.clear();

          let res = open_rom(&PathBuf::from(filename));
          match res {
            Ok(new_emu) => {
              ctx.emu = new_emu;
              ctx.is_paused = false;
              ctx.is_running = true;
              ctx.ms_frame = Duration::from_secs_f32(1.0 / ctx.emu.get_fps());
            },
            Err(e) => eprintln!("{e}"),
          }

          ctx.audio.resume();
        }
        Event::ControllerDeviceAdded { which , .. } => {
          match controller.open(which) {
            Ok(controller) => {
                println!("Found controller: {}\n", controller.name());
                controllers.push(controller);
            }
            Err(_) => eprintln!("A controller was connected, but I couldn't initialize it\n")
          }
        }
        _ => {}
      }
    }

    canvas.clear();
    texture.update(
      None, 
      &ctx.emu.get_screen().buffer, 
      ctx.emu.get_screen().pitch()
    ).unwrap();

    canvas.copy(&texture, None, None).unwrap();
    canvas.present();

    let ms_elapsed = Instant::now() - ms_since_start;
    if ctx.ms_frame > ms_elapsed {
      std::thread::sleep(ctx.ms_frame - ms_elapsed);
    }
  }
}