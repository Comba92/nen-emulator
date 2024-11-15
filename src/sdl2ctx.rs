use sdl2::{controller::GameController, render::{Canvas, TextureCreator}, video::{Window, WindowContext}, EventPump, GameControllerSubsystem, JoystickSubsystem, Sdl, TimerSubsystem, VideoSubsystem};

pub struct Sdl2Context {
  pub ctx: Sdl,
  pub timer: TimerSubsystem,
  pub video_subsytem: VideoSubsystem,
  pub canvas: Canvas<Window>,
  pub texture_creator: TextureCreator<WindowContext>,
  pub events: EventPump,
  pub controller_subsystem: GameControllerSubsystem,
  pub controllers: Vec<GameController>,
}

impl Sdl2Context {
  pub fn new(name: &str, width: u32, height: u32) -> Self {
    let ctx = sdl2::init().expect("Couldn't initialize SDL2");
    let timer = ctx.timer().expect("Couldn't initialize timer subsytem");
    let video_subsystem= ctx.video().expect("Couldn't initialize video subsystem");
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
    
    let mut controllers = Vec::new();
    let controllers_avaible = controller_subsystem.num_joysticks().expect("Couldn't get number of joysticks avaible");

    eprintln!("Found {} joypads", controllers_avaible);
    for i in 0..controllers_avaible {
      if !controller_subsystem.is_game_controller(i) { continue; }
      
      match controller_subsystem.open(i) {
        Ok(controller) => {
          eprintln!("Found controller: {}", controller.name());
          controllers.push(controller);
        }
        Err(e) => eprintln!("Couldn't open controller {i}: {e}"),
      }
    }

    if controllers.is_empty() {
      eprintln!("No game controllers found");
    }
    
    let events = ctx.event_pump().expect("Couldn't get the event pump");
    Self { ctx, video_subsytem: video_subsystem, canvas, events, texture_creator, timer, controller_subsystem, controllers }
  }
}