use std::ptr;

pub use bus::Bus;
pub use cpu::Cpu;
pub use ppu::{Ppu, frame::{self, FramebufIndexed, FramebufRGBA}};
pub use apu::Apu;
pub use joypad::{Joypad, JoypadButton};
pub use mapper::Mapper;

pub mod cpu;
pub mod addr;
pub mod ppu;
pub mod apu;
pub mod dma;
pub mod joypad;
pub mod cart;
pub mod bus;
pub mod mem;
pub mod banks;
pub mod mapper;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct Emulator {
  #[cfg_attr(feature = "serde", serde(skip))]

  pub ctx: EmuCtx,

  bus: Bus,
  cpu: Cpu,
  ppu: Ppu,
  apu: Apu,
  joypad: Joypad,
}

impl Emulator {
  pub fn new(rom: &[u8]) -> Result<Box<Self>, String> {
    let bus = Bus::new(rom)?;
    
    let timing = bus.cart.timing;
    let ppu = Ppu::new(timing);
    let apu = Apu::new(timing);
    let cpu = Cpu::new();
    let joypad = Joypad::default();
    // let oam_dma = OamDma::default();
    // let dmc_dma = DmcDma::default();

    let ctx = EmuCtx::default();

    let mut emu = Box::new(Self {
      ctx, bus, cpu, ppu, apu, joypad,
    });
    emu.bind_pointers();
    emu.cpu.boot();

    Ok(emu)
  }
  
  fn bind_pointers(&mut self) {
    self.ctx.bus = &mut self.bus;
    self.ctx.ppu = &mut self.ppu;
    self.ctx.apu = &mut self.apu;
    self.ctx.joypad = &mut self.joypad;
    self.ctx.cpu = &mut self.cpu;
    // &mut self.oam_dma; 
    // &mut self.dmc_dma;

    let shared_ctx = SharedCtx(&mut self.ctx);
    self.bus.ctx = shared_ctx;
    self.ppu.ctx = shared_ctx;
    self.apu.ctx = shared_ctx;
    self.cpu.ctx = shared_ctx;
  }

  pub fn step_until_vblank(&mut self) {
    loop {
      if self.bus.vblank_poll() { break; }
      self.cpu.step();
    }

    // TODO: consider clearing samples here, and returning (framebuf, samples)
  }

  pub fn reset(&mut self) {
    self.cpu.reset();
    self.ppu.reset();
    self.apu.reset();
  }

  pub fn get_frame_indexed(&self) -> &frame::FrameBuffer<FramebufIndexed> {
    &self.ppu.frame_buf
  }

  pub fn get_frame_rgba(&mut self) -> &frame::FrameBuffer<FramebufRGBA> {
    self.ppu.indexed_framebuf_to_rgba()
  }

  pub fn get_samples(&mut self) -> Vec<f32> {
    self.apu.consume_samples()
  }

  pub fn clear_samples(&mut self) {
    self.apu.discard_samples();
  }

  pub fn get_region_fps(&self) -> f32 {
    self.bus.cart.timing.fps()
  }

  pub const fn get_resolution(&mut self) -> (usize, usize) { (32*8, 30*8) }

  pub fn set_joypad_btn(&mut self, btn: JoypadButton) {
    self.joypad.buttons1.insert(btn);
  }

  pub fn clear_joypad_btn(&mut self, btn: JoypadButton) {
    self.joypad.buttons1.remove(btn);
  }

  pub fn clear_all_joypad_btns(&mut self) {
    self.joypad.buttons1 = JoypadButton::empty();
  }

  pub fn toggle_sprite_limit(&mut self) {
    let limit = &mut self.ppu.oam_sprite_limit;
    *limit = if *limit == 8 { 64 } else { 8 };
  }

  pub fn get_sram(&self) -> Option<&[u8]> {
    let bus = &self.bus;
    bus.cart.has_battery.then_some(&bus.sram)
  }

  pub fn set_sram(&mut self, data: &[u8]) {
    self.bus.sram = data.into();
  }

  pub fn load_savestate(&mut self, other: Emulator) {
    // save prg and chr and memtable in temp values
    let prg = core::mem::take(&mut self.bus.prg);
    let chr = (!self.bus.cart.uses_chr_ram)
      .then(|| core::mem::take(&mut self.bus.chr));
    // the mapping is a collection of fn pointers, we can't serialize them, so we copy them from what we already have
    let mem = core::mem::take(&mut self.bus.cfg.mapping);

    // copy the new emulator
    *self = other;

    // the new emulator is missing prg and chr; we take the temp ones
    self.bus.prg = prg;
    // we only copy the temp chr if it is not chr ram, as that has already been deserialized by serde
    if let Some(chr) = chr { self.bus.chr = chr; }
    self.bus.cfg.mapping = mem;

    // When loading a savestate, we have to rebind all the ctx pointers
    self.bind_pointers();
  }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct EmuCtx {
  #[cfg_attr(feature = "serde", serde(skip))]
  bus: *mut Bus,
  #[cfg_attr(feature = "serde", serde(skip))]
  cpu: *mut Cpu,
  #[cfg_attr(feature = "serde", serde(skip))]
  ppu: *mut Ppu,
  #[cfg_attr(feature = "serde", serde(skip))]
  apu: *mut Apu,
  #[cfg_attr(feature = "serde", serde(skip))]
  pub joypad: *mut Joypad,

  // oam_dma: *mut OamDma,
  // dmc_dma: *mut DmcDma,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EmuCtx {
  fn deserialize<D>(_: D) -> Result<Self, D::Error>
  where
      D: serde::Deserializer<'de> {
    Ok(EmuCtx::default())
  }
}

impl Default for EmuCtx {
  fn default() -> Self {
    Self {
      cpu: ptr::null_mut(),
      ppu: ptr::null_mut(),
      apu: ptr::null_mut(),
      joypad: ptr::null_mut(),
      bus: ptr::null_mut(),
      // oam_dma: ptr::null_mut(),
      // dmc_dma: ptr::null_mut(),
    }
  }
}

#[derive(Clone, Copy)]
pub struct SharedCtx(*mut EmuCtx);
impl Default for SharedCtx {
  fn default() -> Self {
    Self(std::ptr::null_mut())
  }
}

impl SharedCtx {
  // TODO: maybe can be done better?

  fn get(&self) -> &mut EmuCtx {
    unsafe { &mut *self.0 }
  }

  pub fn bus(&self) -> &mut Bus {
    unsafe { &mut*self.get().bus }
  }

  pub fn mapper(&self) -> &mut Box<dyn Mapper> {
    &mut self.bus().mapper
  }

  pub fn cpu(&self) -> &mut Cpu {
    unsafe { &mut*self.get().cpu }
  }

  pub fn ppu(&self) -> &mut Ppu {
    unsafe { &mut*self.get().ppu }
  }

  pub fn apu(&self) -> &mut Apu {
    unsafe { &mut*self.get().apu }
  }

  pub fn joypad(&self) -> &mut Joypad {
    unsafe { &mut*self.get().joypad }
  }

  // pub fn oam_dma(&self) -> &mut OamDma {
  //   unsafe { &mut*self.get().oam_dma }
  // }

  // pub fn dmc_dma(&self) -> &mut DmcDma {
  //   unsafe { &mut*self.get().dmc_dma }
  // }
}