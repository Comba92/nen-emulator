use std::{collections::HashMap, error::Error, fs, io::{BufReader, BufWriter, Read, Write}, path::PathBuf, time::{Duration, Instant}};
use nen_emulator::{joypad::JoypadButton as NesJoypadButton, Emulator};
use sdl2::{audio::{AudioQueue, AudioSpecDesired, AudioStatus}, controller::{Axis, Button}, event::Event, keyboard::Keycode};

enum InputAction {
  Game(NesJoypadButton), Pause, Reset, Mute, Save, Load, SpriteLimit
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
      (Keycode::M, InputAction::Mute),
      (Keycode::NUM_9, InputAction::Save),
      (Keycode::NUM_0, InputAction::Load),
      (Keycode::NUM_1, InputAction::SpriteLimit),
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
      (Button::Guide, InputAction::Pause),
    ]);

    Keymaps { keymap: default_keymap, padmap: default_padmap }
  }
}

fn open_rom(path: &str) -> Result<Box<Emulator>, Box<dyn Error>> {
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

	
  Emulator::new(&bytes)
    .map_err(|msg| msg.into())
}

fn save_sram(ctx: &EmuCtx) {
  if let Some(data) = ctx.emu.get_sram() {
    let path = PathBuf::from(&ctx.rom_path).with_extension("srm");
    let _ = fs::write(path, data)
      .inspect_err(|e| eprintln!("Couldn't save: {e}"));
  }
}

fn load_sram(ctx: &mut EmuCtx) {
  let path = PathBuf::from(&ctx.rom_path).with_extension("srm");
  if let Ok(data) = fs::read(path) {
    ctx.emu.set_sram(&data);
  }
}

#[cfg(feature = "serde")]
fn save_state(ctx: &EmuCtx) {
  let path = PathBuf::from(&ctx.rom_path).with_extension("nensv");
  let writer = BufWriter::new(fs::File::create(path).expect("Couldn't create savestate file"));
  let _ = pot::to_writer(&ctx.emu, writer)
    .inspect_err(|e| eprintln!("Couldn't write savestate to file: {e}"));
  // let s = ron::to_string(&ctx.emu).unwrap();
  // writer.write_fmt(format_args!("{s}")).unwrap();
}

#[cfg(not(feature = "serde"))]
fn save_state(ctx: &EmuCtx) {
  eprintln!("serde feature must be enabled for savestates");
}


#[cfg(feature = "serde")]
fn load_state(ctx: &mut EmuCtx) {
  let path = PathBuf::from(&ctx.rom_path).with_extension("nensv");
  let savestate = fs::File::open(path);

  match savestate {
    Ok(file) => {
      let reader = BufReader::new(file);
      let new_emu = pot::from_reader(reader);
      match new_emu {
        Ok(new_emu) => {
          ctx.emu.load_savestate(new_emu);
        }
        Err(e) => eprintln!("Couldn't deserialize emulator object: {e:?}")
      }
      // let mut s = String::new();
      // reader.read_to_string(&mut s).unwrap();
      // let new_emu = ron::from_str(&s).unwrap();
      // ctx.emu.load_savestate(new_emu);
    }
    Err(e) => eprintln!("Couldn't read savestate from file: {e:?}")
  }
}

#[cfg(not(feature = "serde"))]
fn load_state(ctx: &mut EmuCtx) {
  eprintln!("serde feature must be enabled for savestates");
}

fn handle_input(keys: &Keymaps, event: &Event, ctx: &mut EmuCtx) {
  let emu = &mut ctx.emu;

  match event {
    Event::KeyDown { keycode, .. } 
    | Event::KeyUp { keycode, .. } => {
      if let Some(keycode) = keycode {
        if let Some(action) = keys.keymap.get(&keycode) {
          match (action, event) {
            (InputAction::Game(button), Event::KeyDown {..}) => emu.set_joypad_btn(*button),
            (InputAction::Game(button), Event::KeyUp {..}) => emu.clear_joypad_btn(*button),
            (InputAction::Pause, Event::KeyDown {..}) => {
              ctx.is_paused = !ctx.is_paused;
              ctx.is_muted = ctx.audio.status() == AudioStatus::Playing;
              match &ctx.audio.status() {
                AudioStatus::Playing => ctx.audio.pause(),
                _=> ctx.audio.resume(),
              }
            },
            (InputAction::Reset, Event::KeyDown {..}) => emu.reset(),
            (InputAction::Mute, Event::KeyDown {..}) => {
              ctx.is_muted = ctx.audio.status() != AudioStatus::Playing;
              match &ctx.audio.status() {
                AudioStatus::Playing => ctx.audio.pause(),
                _=> ctx.audio.resume(),
              }
            }
            (InputAction::Save, Event::KeyDown {..}) => save_state(ctx),
            (InputAction::Load, Event::KeyDown {..}) => load_state(ctx),
            (InputAction::SpriteLimit, Event::KeyDown {..}) => ctx.emu.toggle_sprite_limit(),
            _ => {}
          }
        }
      }
    }

    Event::ControllerButtonDown { button, .. } 
    | Event::ControllerButtonUp { button, .. }  => {
      if let Some(action) = keys.padmap.get(&button) {
        match (action, event) {
          (InputAction::Game(button), Event::ControllerButtonDown {..}) => emu.set_joypad_btn(*button),
          (InputAction::Game(button), Event::ControllerButtonUp {..}) => emu.clear_joypad_btn(*button),
          (InputAction::Pause, Event::ControllerButtonDown {..}) => {
            ctx.is_paused = !ctx.is_paused;
            ctx.is_muted = ctx.audio.status() == AudioStatus::Playing;
            match &ctx.audio.status() {
              AudioStatus::Playing => ctx.audio.pause(),
              _=> ctx.audio.resume(),
            }
          }
          (InputAction::Reset, Event::ControllerButtonDown {..}) => emu.reset(),
          (InputAction::Mute, Event::KeyDown {..}) => {
            ctx.is_muted = ctx.audio.status() != AudioStatus::Playing;
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
      if *value > AXIS_DEAD_ZONE { emu.set_joypad_btn(NesJoypadButton::right); }
      else if *value < -AXIS_DEAD_ZONE { emu.set_joypad_btn(NesJoypadButton::left); }
      else {
        emu.clear_joypad_btn(NesJoypadButton::left);
        emu.clear_joypad_btn(NesJoypadButton::right);
      }
    }
    Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
      if *value > AXIS_DEAD_ZONE { emu.set_joypad_btn(NesJoypadButton::down); }
      else if *value < -AXIS_DEAD_ZONE { emu.set_joypad_btn(NesJoypadButton::up); }
      else {
        emu.clear_joypad_btn(NesJoypadButton::up);
        emu.clear_joypad_btn(NesJoypadButton::down);
      }
    }
    _ => {}
  }
}

struct EmuCtx {
  emu: Box<Emulator>,
  is_paused: bool,
  is_running: bool,
  is_muted: bool,
  audio: AudioQueue<f32>,
  ms_frame: Duration,
  rom_path: String,
}

fn main() {
  const SCALE: f32 = 3.0;
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

  let emu = Box::new(Emulator::default());

  let mut texture = texture_creator.create_texture_streaming(
    sdl2::pixels::PixelFormatEnum::RGBA32,
    emu.get_frame().width as u32, emu.get_frame().height as u32
  ).unwrap();

  let desired_spec = AudioSpecDesired {
    freq: Some(44100),
    channels: Some(1),
    samples: None,
  };

  let audio_dev = audio
    .open_queue::<f32, _>(None, &desired_spec).unwrap();

  let mut ctx = EmuCtx {
    ms_frame: Duration::from_secs_f32(1.0 / 60.0),
    is_paused: true,
    is_running: false,
    is_muted: false,
    audio: audio_dev,
    emu,
    rom_path: String::new(),
  };

  const SAMPLES_PER_FRAME: u32 = 735;
  
  'running: loop {
    let ms_since_start = Instant::now();

    if !ctx.is_paused {
      ctx.emu.step_until_vblank();

      // if you don't have enough audio, we run for another frame
      if !ctx.is_muted && ctx.audio.size() < SAMPLES_PER_FRAME*3 {
        ctx.emu.step_until_vblank();
      }

      let samples = ctx.emu.get_samples();
      if !ctx.is_muted {
        ctx.audio.queue_audio(&samples).unwrap();
      }
    }

    for event in events.poll_iter() {
      if ctx.is_running {
        handle_input(&keymaps, &event, &mut ctx);
      }

      match event {
        Event::Quit { .. } => {
          save_sram(&ctx);
          break 'running;
        }
        Event::KeyDown { keycode, .. } => {
          if let Some(keycode) = keycode {
            if keycode == Keycode::Return {
              let fullscreen = match canvas.window().fullscreen_state() {
                sdl2::video::FullscreenType::Off => sdl2::video::FullscreenType::Desktop,
                _ => sdl2::video::FullscreenType::Off
              };
              canvas.window_mut().set_fullscreen(fullscreen).unwrap();
            }
          }
        }
        Event::DropFile { filename, .. } => {
          ctx.audio.pause();
          ctx.audio.clear();

          let res = open_rom(&filename);
          match res {
            Ok(new_emu) => {
              save_sram(&ctx);

              ctx.rom_path = filename;
              ctx.emu = new_emu;
              ctx.is_paused = false;
              ctx.is_running = true;
              ctx.ms_frame = Duration::from_secs_f32(1.0 / ctx.emu.get_region_fps());

              load_sram(&mut ctx);
            },
            Err(e) => eprintln!("{e}"),
          }

          if !ctx.is_muted { ctx.audio.resume(); }
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

    texture.with_lock(None, |pixels, _pitch| {
      pixels.copy_from_slice(&ctx.emu.get_frame().buffer);
    }).unwrap();

    canvas.copy(&texture, None, None).unwrap();
    canvas.present();

    let ms_elapsed = Instant::now() - ms_since_start;
    if ctx.ms_frame > ms_elapsed {
      std::thread::sleep(ctx.ms_frame - ms_elapsed);
    }
  }
}
