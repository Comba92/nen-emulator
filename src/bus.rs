use log::debug;
use crate::{apu::Apu, cart::{Cart, INesHeader, Mirroring}, joypad::Joypad, mapper::CartMapper, mem::Memory, ppu::Ppu};

#[derive(Debug)]
enum BusDst {
  Ram, Ppu, Apu, SRam, Prg, Joypad1, Joypad2, NoImpl
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
}
// TODO: consider moving VRAM here

impl Memory for Bus {
  fn read(&mut self, addr: u16) -> u8 {
    let (dst, addr) = self.map_address(addr);
    match dst {
      BusDst::Ram => self.ram[addr],
      BusDst::Ppu => self.ppu.read_reg(addr as u16),
      BusDst::Apu => self.apu.read_reg(addr as u16),
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
      BusDst::SRam => self.sram[addr] = val,
      BusDst::Prg => self.mapper.borrow_mut().write_prg(&mut self.prg, addr, val),
      BusDst::NoImpl => debug!("Write to {addr:04X} not implemented")
    }
  }

  fn poll_nmi(&mut self) -> bool {
    self.ppu.nmi_requested.take().is_some()
  }

  fn poll_irq(&mut self) -> bool {
    self.mapper.borrow_mut().poll_irq()
    || self.apu.frame_irq_requested.take().is_some()
  }
  
  fn tick(&mut self) {
    for _ in 0..3 { self.ppu.step(); }
    self.apu.step();
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
    }
  }

  fn map_address(&self, addr: u16) -> (BusDst, usize) {
    match addr {
      0x0000..=0x1FFF => (BusDst::Ram, addr as usize & 0x07FF),
      0x2000..=0x3FFF => (BusDst::Ppu, addr as usize & 0x2007),
      0x4000..=0x4013 | 0x4015 => (BusDst::Apu, addr as usize),
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
