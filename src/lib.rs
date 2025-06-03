use std::ptr;

use bus::Bus;
use cpu::Cpu;
use joypad::Joypad;
use ppu::Ppu;
use apu::Apu;
use joypad::JoypadButton;
use mapper::Mapper;

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
    // BINDING ORDER IS IMPORTANT (cpu should be last) !!!

    self.ctx.bind(
      &mut self.bus, 
      &mut self.cpu, 
      &mut self.ppu, 
      &mut self.apu, 
      // &mut self.oam_dma, 
      // &mut self.dmc_dma, 
      &mut self.joypad);

    let shared_ctx = SharedCtx(&mut self.ctx);
    
    self.bus.bind(shared_ctx);
    self.ppu.bind(shared_ctx);
    self.apu.bind(shared_ctx);
    self.cpu.bind(shared_ctx);
  }

  pub fn step_until_vblank(&mut self) {
    loop {
      if self.bus.vblank_poll() { break; }
      self.cpu.step();
    }
  }

  pub fn reset(&mut self) {
    self.cpu.reset();
    self.ppu.reset();
    self.apu.reset();
  }

  pub fn get_frame(&self) -> &ppu::frame::FrameBuffer {
    &self.ppu.frame
  }

  pub fn get_samples(&mut self) -> Vec<f32> {
    self.apu.consume_samples()
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
  // oam_dma: *mut OamDma,
  // dmc_dma: *mut DmcDma,
  #[cfg_attr(feature = "serde", serde(skip))]

  pub joypad: *mut Joypad,
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
      // oam_dma: ptr::null_mut(),
      // dmc_dma: ptr::null_mut(),
      joypad: ptr::null_mut(),
      bus: ptr::null_mut(),
    }
  }
}

impl EmuCtx {
  fn bind(
    &mut self, 
    bus: *mut Bus,
    cpu: *mut Cpu, 
    ppu: *mut Ppu,
    apu: *mut Apu,
    // oam_dma: *mut OamDma,
    // dmc_dma: *mut DmcDma,
    joypad: *mut Joypad
  )
{
    self.bus = bus;
    self.cpu = cpu;
    self.ppu = ppu;
    self.apu = apu;
    // self.oam_dma = oam_dma;
    // self.dmc_dma = dmc_dma;
    self.joypad = joypad;
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

  // pub fn oam_dma(&self) -> &mut OamDma {
  //   unsafe { &mut*self.get().oam_dma }
  // }

  // pub fn dmc_dma(&self) -> &mut DmcDma {
  //   unsafe { &mut*self.get().dmc_dma }
  // }

  pub fn joypad(&self) -> &mut Joypad {
    unsafe { &mut*self.get().joypad }
  }
}