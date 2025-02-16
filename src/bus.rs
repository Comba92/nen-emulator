use crate::{apu::Apu, cart::{Cart, ConsoleTiming, SharedCart}, dma::{Dma, OamDma}, joypad::Joypad, mem::Memory, ppu::Ppu};

#[derive(Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum EmulatorTiming { #[default] NSTC, PAL }
impl From<ConsoleTiming> for EmulatorTiming {
  fn from(value: ConsoleTiming) -> Self {
    match value {
      ConsoleTiming::PAL => EmulatorTiming::PAL,
      _ => EmulatorTiming::NSTC,
    }
  }
}
const PPU_STEPPINGS: [fn(&mut Bus); 2] = [Bus::ppu_step_nstc, Bus::ppu_step_pal];


#[derive(serde::Serialize, serde::Deserialize)]
pub struct Bus {
  pub ram: Box<[u8]>,
  pub cart: SharedCart,
  pub ppu: Ppu,
  ppu_pal_cycles: u8,
  timing: EmulatorTiming,

  pub apu: Apu,
  pub joypad: Joypad,
  pub oam_dma: OamDma,
}

impl Memory for Bus {
  fn read(&mut self, addr: u16) -> u8 {
    let dev = addr >> 13;
    let handler = self.cart.mapping().cpu_reads[dev as usize];
    handler(self, addr)
  }

  fn write(&mut self, addr: u16, val: u8) {
    let dev = addr >> 13;
    let handler = self.cart.mapping().cpu_writes[dev as usize];
    handler(self, addr, val);
  }

  fn nmi_poll(&mut self) -> bool {
    // https://www.nesdev.org/wiki/NMI
    let res = self.ppu.nmi_requested.take().is_some();
		
    if self.ppu.nmi_tmp.is_some() {
			self.ppu.nmi_requested = self.ppu.nmi_tmp.take();
		}

    res
  }

  fn irq_poll(&mut self) -> bool {
    self.cart.as_mut().mapper.poll_irq()
    || self.apu.frame_irq_flag.is_some()
    || self.apu.dmc.irq_flag.is_some()
  }
  
  fn tick(&mut self) {
    PPU_STEPPINGS[self.timing as usize](self);
    self.apu.step();
    self.cart.as_mut().mapper.notify_cpu_cycle();
  }

  fn handle_dma(&mut self) -> bool {
    if self.apu.dmc.reader.is_transfering() && self.apu.dmc.is_empty() {
      self.tick();
      self.tick();

      let addr = self.apu.dmc.reader.current();
      let to_write = self.read(addr);
      self.tick();
      self.apu.dmc.load_sample(to_write);
      self.tick();

      return true;
    } else if self.oam_dma.is_transfering() {
      let addr = self.oam_dma.current();
      let to_write = self.read(addr);
      self.tick();
      self.write(0x2004, to_write);
      self.tick();

      return true;
    }

    return false;
  }
}

impl Drop for Bus {
  fn drop(&mut self) {
    // This is needed, as we're manually managing a cart pointer to heap
    unsafe {
      drop(Box::from_raw(self.cart.0))
    }
  }
}

impl Bus {
  pub fn new(cart: Cart) -> Self {
    let timing = EmulatorTiming::from(cart.header.timing);

    let shared_cart = SharedCart::new(cart); 

    let ppu = Ppu::new(shared_cart.clone());
    let apu = Apu::new(shared_cart.clone());

    Self {
      timing,
      ram: vec![0; 0x800].into_boxed_slice(), 
      ppu,
      ppu_pal_cycles: 0,
      apu,
      cart: shared_cart,
      joypad: Joypad::new(),
      oam_dma: OamDma::default(),
    }
  }

  fn ppu_step_nstc(&mut self) {
    for _ in 0..3 { self.ppu.step(); }
  }

  fn ppu_step_pal(&mut self) {
    for _ in 0..3 { self.ppu.step(); }
    
    // PPU is run for 3.2 cycles on PAL
    self.ppu_pal_cycles += 1;
    if self.ppu_pal_cycles >= 5 {
      self.ppu_pal_cycles = 0;
      self.ppu.step();
    }
  }

  pub fn poll_vblank(&mut self) -> bool {
    self.ppu.frame_ready.take().is_some()
  }
}

impl Bus {
  pub fn prg_read(&mut self, addr: u16) -> u8 {
    let cart = self.cart.as_mut();
    cart.prg[cart.mapper.prg_translate(&mut cart.cfg, addr)]
  }

  pub fn prg_write(&mut self, addr: u16, val: u8) {
    let cart = self.cart.as_mut();
    cart.mapper.prg_write(&mut cart.cfg, addr as usize, val);
  }

  pub fn sram_read(&mut self, addr: u16) -> u8 {
    let cart = self.cart.as_mut();
    cart.sram[cart.mapper.sram_translate(&mut cart.cfg, addr)]
  }

  pub fn sram_write(&mut self, addr: u16, val: u8) {
    let cart = self.cart.as_mut();
    cart.sram[cart.mapper.sram_translate(&mut cart.cfg, addr)] = val;
  }
}