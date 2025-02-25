
use crate::{apu::Apu, cart::{Cart, ConsoleTiming, SharedCart}, dma::{Dma, OamDma}, joypad::Joypad, mem::Memory, ppu::Ppu};

#[derive(Debug)]
enum BusDst {
  Ram, Ppu, Apu, SRam, Cart, Prg, Joypad1, Joypad2, OamDma, DmcDma, NoImpl
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Bus {
  timing: ConsoleTiming,
  ram: Box<[u8]>,
  pub cart: SharedCart,
  pub ppu: Ppu,
  ppu_pal_cycles: u8,

  pub apu: Apu,
  pub joypad: Joypad,
  pub oam_dma: OamDma,
}

fn map_address(addr: u16) -> (BusDst, usize) {
  let addr = addr as usize;
  match addr {
    0x0000..=0x1FFF => (BusDst::Ram, addr & 0x07FF),
    0x2000..=0x3FFF => (BusDst::Ppu, addr & 0x2007),
    0x4000..=0x4013 => (BusDst::Apu, addr),
    0x4014 => (BusDst::OamDma, addr),
    0x4015 => (BusDst::DmcDma, addr),
    0x4016 => (BusDst::Joypad1, addr),
    0x4017 => (BusDst::Joypad2, addr),
    0x4020..=0x5FFF => (BusDst::Cart, addr),
    0x6000..=0x7FFF => (BusDst::SRam, addr),
    // We pass it as is to the mapper, for convenience
    0x8000..=0xFFFF => (BusDst::Prg, addr),
    _ => (BusDst::NoImpl, addr)
  }
}

impl Memory for Bus {
  fn read(&mut self, addr: u16) -> u8 {
    let (dst, addr) = map_address(addr);
    match dst {
      BusDst::Ram => self.ram[addr],
      BusDst::Ppu => self.ppu.read_reg(addr as u16),
      BusDst::Apu | BusDst::DmcDma => self.apu.read_reg(addr as u16),
      BusDst::Joypad1 => self.joypad.read1(),
      BusDst::Joypad2 => self.joypad.read2(),
      BusDst::Cart | BusDst::SRam | BusDst::Prg  => self.cart.as_mut().prg_read(addr),
      _ => { 0 }
    }
  }

  fn write(&mut self, addr: u16, val: u8) {
    let (dst, addr) = map_address(addr);
    match dst {
      BusDst::Ram => self.ram[addr] = val,
      BusDst::Ppu => self.ppu.write_reg(addr as u16, val),
      BusDst::Apu => self.apu.write_reg(addr as u16, val),
      BusDst::Joypad2 => {
        self.apu.write_reg(addr as u16, val);
        self.joypad.write(val);
      }
      BusDst::Joypad1 => self.joypad.write(val),
      BusDst::OamDma => {
        self.oam_dma.init(val);
        self.tick();
      }
      BusDst::DmcDma => {
        self.apu.write_reg(addr as u16, val);
        self.tick();
      }
      BusDst::Cart | BusDst::SRam | BusDst::Prg => self.cart.as_mut().prg_write(addr, val),
      BusDst::NoImpl => {}
    }
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
    match self.timing {
      ConsoleTiming::PAL => self.ppu_step_pal(),
      _ => self.ppu_step_nstc(),
    };

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
    let timing = cart.header.timing;
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
