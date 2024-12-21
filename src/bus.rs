use log::debug;
use crate::{apu::Apu, cart::{Cart, INesHeader, Mirroring}, dma::{Dma, OamDma}, joypad::Joypad, mapper::CartMapper, mem::Memory, ppu::Ppu};

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

impl Memory for Bus {
  fn read(&mut self, addr: u16) -> u8 {
    let (dst, addr) = self.map_address(addr);
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
    let (dst, addr) = self.map_address(addr);
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
    self.ppu.nmi_requested.take().is_some()
  }

  fn irq_poll(&mut self) -> bool {
    self.mapper.borrow_mut().poll_irq()
    || self.apu.frame_irq_flag.take().is_some()
    // https://www.nesdev.org/wiki/APU_DMC#Memory_reader
    || self.apu.dmc.irq_flag.is_some()
  }
  
  fn tick(&mut self) {
    for _ in 0..3 { self.ppu.step(); }
    self.apu.step();
  }

  fn is_dma_transfering(&self) -> bool {
    (self.apu.dmc.reader.is_transfering() && self.apu.dmc.is_empty())
      || self.oam_dma.is_transfering()
  }

  fn handle_dma(&mut self) {
    if self.apu.dmc.reader.is_transfering() && self.apu.dmc.is_empty() {
      // println!("Doing dmc dma");
      // println!("Is empty: {}", self.apu.dmc.is_empty());
      // println!("DMA addr: {}", self.apu.dmc.reader.addr);
      // println!("DMA length: {}", self.apu.dmc.reader.remaining);

      self.tick();
      self.tick();

      let addr = self.apu.dmc.reader.current();
      let to_write = self.read(addr);
      self.tick();
      self.apu.dmc.load_sample(to_write);

      // println!("Is empty after: {}", self.apu.dmc.is_empty());
      // println!("DMA addr after: {}", self.apu.dmc.reader.addr);
      // println!("DMA length after: {}", self.apu.dmc.reader.remaining);

      if !self.apu.dmc.reader.is_transfering() {
        if self.apu.dmc.loop_enabled {
          self.apu.dmc.restart_dma();
        } else if self.apu.dmc.irq_enabled {
          self.apu.dmc.irq_flag = Some(());
        }
      }
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

  fn map_address(&self, addr: u16) -> (BusDst, usize) {
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

  pub fn poll_vblank(&mut self) -> bool {
    self.ppu.vblank_started.take().is_some()
  }

  pub fn poll_sample(&mut self) -> Option<i16> {
    self.apu.current_sample.take()
  }

  pub fn mirroring(&self) -> Mirroring {
    if let Some(mirroring) = self.mapper.borrow().mirroring() {
      mirroring
    } else {
      self.cart.nametbl_mirroring
    }
  }
}
