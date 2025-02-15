use crate::{apu::Apu, cart::{Cart, ConsoleTiming, SharedCart}, dma::{Dma, OamDma}, joypad::Joypad, mem::Memory, ppu::Ppu};

// https://www.nesdev.org/wiki/CPU_memory_map
const CPU_MAPPING_READS: [fn(&mut Bus, u16) -> u8; 8] = [
  |bus: &mut Bus, addr: u16| bus.ram[addr as usize & 0x7FF],
  |bus: &mut Bus, addr: u16| bus.ppu.read_reg(addr & 0x2007),
  |bus: &mut Bus, addr: u16| {
    match addr {
      0x4000..=0x4013 => bus.apu.read_reg(addr),
      0x4016 => bus.joypad.read1(),
      0x4017 => bus.joypad.read2(),
      0x4020..=0x5FFF => bus.cart.as_mut().prg_read(addr as usize),
      _ => 0,
    }
  },
  |bus: &mut Bus, addr: u16| bus.cart.as_mut().prg_read(addr as usize),
  |bus: &mut Bus, addr: u16| bus.cart.as_mut().prg_read(addr as usize),
  |bus: &mut Bus, addr: u16| bus.cart.as_mut().prg_read(addr as usize),
  |bus: &mut Bus, addr: u16| bus.cart.as_mut().prg_read(addr as usize),
  |bus: &mut Bus, addr: u16| bus.cart.as_mut().prg_read(addr as usize),
];


const CPU_MAPPING_WRITES: [fn(&mut Bus, u16, u8); 8] = [
  |bus: &mut Bus, addr: u16, val: u8| bus.ram[addr as usize & 0x7FF] = val,
  |bus: &mut Bus, addr: u16, val: u8| bus.ppu.write_reg(addr & 0x2007, val),
  |bus: &mut Bus, addr: u16, val: u8| {
    match addr {
      0x4000..=0x4013 => bus.apu.write_reg(addr as u16, val),
      0x4017 => {
        bus.apu.write_reg(addr as u16, val);
        bus.joypad.write(val);
      }
      0x4016 => bus.joypad.write(val),
      0x4014 => {
        bus.oam_dma.init(val);
        bus.tick();
      }
      0x4015 => {
        bus.apu.write_reg(addr as u16, val);
        bus.tick();
      }
      0x4020..=0x5FFF => bus.cart.as_mut().prg_write(addr as usize, val),
      _ => {}
    }
  },
  |bus: &mut Bus, addr: u16, val: u8| bus.cart.as_mut().prg_write(addr as usize, val),
  |bus: &mut Bus, addr: u16, val: u8| bus.cart.as_mut().prg_write(addr as usize, val),
  |bus: &mut Bus, addr: u16, val: u8| bus.cart.as_mut().prg_write(addr as usize, val),
  |bus: &mut Bus, addr: u16, val: u8| bus.cart.as_mut().prg_write(addr as usize, val),
  |bus: &mut Bus, addr: u16, val: u8| bus.cart.as_mut().prg_write(addr as usize, val),
];

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

#[allow(unused)]
#[derive(Debug)]
enum BusDst {
  Ram, Ppu, Apu, SRam, Cart, Prg, Joypad1, Joypad2, OamDma, DmcDma, NoImpl
}

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

#[allow(unused)]
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
    self.read_branchless(addr)
  }

  fn write(&mut self, addr: u16, val: u8) {
    self.write_branchless(addr, val);
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

  #[allow(unused)]
  fn read_branching(&mut self, addr: u16) -> u8 {
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

  
  fn read_branchless(&mut self, addr: u16) -> u8 {
    let dev = addr >> 13;
    let handler = self.cart.mapping().cpu_reads[dev as usize];
    handler(self, addr)
  }

  #[allow(unused)]
  fn write_branching(&mut self, addr: u16, val: u8) {
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

  fn write_branchless(&mut self, addr: u16, val: u8) {
    let dev = addr >> 13;
    let handler = self.cart.mapping().cpu_writes[dev as usize];
    handler(self, addr, val);
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
