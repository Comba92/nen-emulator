use log::debug;
use crate::{apu::Apu, cart::{Cart, INesHeader}, dma::{Dma, OamDma}, joypad::Joypad, mapper::CartMapper, mem::Memory, ppu::Ppu};

#[derive(Debug)]
enum BusDst {
  Ram, Ppu, Apu, SRam, Prg, Joypad1, Joypad2, OamDma, DmcDma, NoImpl
}

pub struct Bus {
  ram: [u8; 0x800],
  sram: [u8; 0x2000],
  prg: Vec<u8>,
  pub cart: INesHeader,
  pub mapper: CartMapper,
  pub ppu: Ppu,
  pub apu: Apu,
  pub joypad: Joypad,
  pub oam_dma: OamDma,
}
// TODO: consider moving VRAM here

fn map_address(addr: u16) -> (BusDst, usize) {
  match addr {
    0x0000..=0x1FFF => (BusDst::Ram, addr as usize & 0x07FF),
    0x2000..=0x3FFF => (BusDst::Ppu, addr as usize & 0x2007),
    0x4000..=0x4013 => (BusDst::Apu, addr as usize),
    0x4014 => (BusDst::OamDma, addr as usize),
    0x4015 => (BusDst::DmcDma, addr as usize),
    0x4016 => (BusDst::Joypad1, addr as usize),
    0x4017 => (BusDst::Joypad2, addr as usize),
    0x6000..=0x7FFF => (BusDst::SRam, addr as usize - 0x6000),
    // We pass it as is to the mapper, for convenience
    0x8000..=0xFFFF => (BusDst::Prg, addr as usize),
    _ => (BusDst::NoImpl, addr as usize)
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
      BusDst::SRam => self.sram[addr],
      BusDst::Prg => self.mapper.borrow_mut().read_prg(&self.prg, addr),
      _ => { debug!("Read to {addr:04X} not implemented"); 0 }
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
      BusDst::SRam => self.sram[addr] = val,
      BusDst::Prg => self.mapper.borrow_mut().write_prg(&mut self.prg, addr, val),
      BusDst::NoImpl => debug!("Write to {addr:04X} not implemented")
    }
  }

  fn nmi_poll(&mut self) -> bool {
    // https://www.nesdev.org/wiki/NMI
    self.ppu.nmi_requested.take().is_some()
  }

  fn irq_poll(&mut self) -> bool {
    self.mapper.borrow_mut().poll_irq()
    || self.apu.frame_irq_flag.is_some()
    || self.apu.dmc.irq_flag.is_some()
  }
  
  fn tick(&mut self) {
    for _ in 0..3 { self.ppu.step(); }
    self.apu.step();
    self.mapper.borrow_mut().notify_cpu_cycle();
  }

  fn is_dma_transfering(&self) -> bool {
    (self.apu.dmc.reader.is_transfering() && self.apu.dmc.is_empty())
      || self.oam_dma.is_transfering()
  }

  fn handle_dma(&mut self) {
    if self.apu.dmc.reader.is_transfering() && self.apu.dmc.is_empty() {
      self.tick();
      self.tick();

      let addr = self.apu.dmc.reader.current();
      let to_write = self.read(addr);
      self.tick();
      self.apu.dmc.load_sample(to_write);
      self.tick();
    } else if self.oam_dma.is_transfering() {
      let addr = self.oam_dma.current();
      let to_write = self.read(addr);
      self.tick();
      self.write(0x2004, to_write);
      self.tick();
    }
  }
}

impl Bus {
  pub fn new(cart: Cart) -> Self {
    let ppu = Ppu::new(
      cart.chr_rom,
      cart.mapper.clone(),
      cart.header.nametbl_mirroring
    );

    Self {
      ram: [0; 0x800], 
      sram: [0; 0x2000],
      ppu,
      apu: Apu::new(),
      cart: cart.header,
      prg: cart.prg_rom,
      mapper: cart.mapper,
      joypad: Joypad::new(),
      oam_dma: OamDma::default(),
    }
  }

  pub fn poll_vblank(&mut self) -> bool {
    self.ppu.vblank_started.take().is_some()
  }

  pub fn poll_sample(&mut self) -> Option<i16> {
    self.apu.current_sample.take()
  }
}
