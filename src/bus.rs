use crate::{
  banks::MemConfig,
  cart::{self, CartHeader, ConsoleTiming},
  dma::Dma,
  mapper::{self, DummyMapper, Mapper},
  SharedCtx,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Clone, Copy)]
pub enum EmuTiming {
  #[default]
  NSTC,
  PAL,
}
impl From<ConsoleTiming> for EmuTiming {
  fn from(value: ConsoleTiming) -> Self {
    match value {
      ConsoleTiming::PAL => EmuTiming::PAL,
      _ => EmuTiming::NSTC,
    }
  }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Bus {
  // TODO: Should ctx own header?
  pub cart: CartHeader,
  #[cfg_attr(feature = "serde", serde(skip))]
  pub ctx: SharedCtx,

  // TODO: could ram be owned by CPU??
  pub ram: Box<[u8]>,
  #[cfg_attr(feature = "serde", serde(skip))]
  pub prg: Box<[u8]>,
  pub chr: Box<[u8]>,
  pub vram: Box<[u8]>,
  pub sram: Box<[u8]>,

  pub cfg: MemConfig,
  ppu_pal_cycles: u8,
  ppu_timing: EmuTiming,

  // TODO: should ctx own mapper?
  pub mapper: Box<dyn Mapper>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Bus {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    use serde::ser::SerializeStruct;
    let mut se = serializer.serialize_struct("Bus", 9)?;

    // we do not care to serialize prg and ctx
    se.skip_field("prg")?;
    se.skip_field("ctx")?;

    se.serialize_field("cart", &self.cart)?;
    se.serialize_field("ram", &self.ram)?;

    // we only serialize chr if it is chr ram
    if self.cart.uses_chr_ram {
      se.serialize_field("chr", &self.chr)?;
    } else {
      se.serialize_field("chr", &Vec::<u8>::new().into_boxed_slice())?;
    }
    se.serialize_field("vram", &self.vram)?;
    se.serialize_field("sram", &self.sram)?;
    se.serialize_field("cfg", &self.cfg)?;
    se.serialize_field("ppu_pal_cycles", &self.ppu_pal_cycles)?;
    se.serialize_field("ppu_timing", &self.ppu_timing)?;
    se.serialize_field("mapper", &self.mapper)?;

    se.end()
  }
}

impl Default for Bus {
  fn default() -> Self {
    Self {
      cart: CartHeader::default(),
      ctx: SharedCtx::default(),

      ram: Default::default(),
      prg: Default::default(),
      chr: Default::default(),
      vram: Default::default(),
      sram: Default::default(),

      ppu_pal_cycles: Default::default(),
      ppu_timing: Default::default(),

      cfg: MemConfig::default(),
      mapper: Box::new(DummyMapper::default()),
    }
  }
}

impl Bus {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    let header =
      CartHeader::new(&rom).map_err(|e| format!("Not a valid iNES/Nes2.0 rom: {e}"))?;

    println!("Loaded NES ROM: {:#?}", header);

    let mut cfg = MemConfig::new(&header);
    let mapper = mapper::new_mapper(&header, &mut cfg)?;

    let prg_start = cart::HEADER_SIZE + if header.has_trainer { 512 } else { 0 };
    let chr_start = prg_start + header.prg_size;

    let prg = rom[prg_start..chr_start].to_vec().into_boxed_slice();
    let chr = if header.uses_chr_ram {
      vec![0; header.chr_ram_size]
    } else {
      rom[chr_start..chr_start + header.chr_size].to_vec()
    }
    .into_boxed_slice();

    let sram_size = header.sram_real_size();
    let sram = vec![0; sram_size].into_boxed_slice();

    let vram_size = if header.has_alt_mirroring {
      4 * 1024
    } else {
      2 * 1024
    };
    let vram = vec![0; vram_size].into_boxed_slice();

    let mut ram = vec![0; 2 * 1024].into_boxed_slice();
    let _ = getrandom::fill(&mut ram)
      .inspect_err(|e| eprintln!("Couldn't initialize RAM with random values: {e}"));

    let ppu_timing = header.timing.into();

    Ok(Self {
      cart: header,
      prg,
      chr,
      vram,
      sram,
      ram,
      mapper,
      cfg,
      ppu_timing,
      ..Default::default()
    })
  }
}

impl Bus {
  pub fn cpu_read(&mut self, addr: u16) -> u8 {
    // notice that: 16bits of address -> 16 - 13 = 3, 2^3 = 8 which is the size of the handlers array.
    // (I THINK, SHOULD BE CONFIRMED)

    let device = addr >> 13;
    let handler = self.cfg.mapping.cpu_reads[device as usize];
    handler(self, addr)
  }

  pub fn cpu_write(&mut self, addr: u16, val: u8) {
    let dev = addr >> 13;
    let handler = self.cfg.mapping.cpu_writes[dev as usize];
    handler(self, addr, val);
  }

  pub fn ppu_read(&mut self, addr: u16) -> u8 {
    let dev = (addr >> 10) & 0xf;
    let handler = self.cfg.mapping.ppu_reads[dev as usize];
    handler(self, addr & 0x3fff)
  }

  pub fn ppu_write(&mut self, addr: u16, val: u8) {
    let dev = (addr >> 10) & 0xf;
    let handler = self.cfg.mapping.ppu_writes[dev as usize];
    handler(self, addr & 0x3fff, val);
  }

  // TODO: these functions dont have to be here
  pub fn tick(&mut self) {
    const PPU_STEPPINGS: [fn(&mut Bus); 2] = [Bus::ppu_step_nstc, Bus::ppu_step_pal];
    PPU_STEPPINGS[self.ppu_timing as usize](self);
    self.ctx.apu().tick();
    self.mapper.notify_cpu_cycle();
  }

  pub fn handle_dmc(&mut self) {
    while self.ctx.apu().dmc.reader.is_transfering() && self.ctx.apu().dmc.is_empty() {
      self.tick();
      self.tick();

      let addr = self.ctx.apu().dmc.reader.current();
      let to_write = self.cpu_read(addr);
      self.tick();
      self.ctx.apu().dmc.load_sample(to_write);
      self.tick();
    }
  }

  fn ppu_step_nstc(&mut self) {
    let ppu = self.ctx.ppu();
    ppu.tick();
    ppu.tick();
    ppu.tick();
  }

  fn ppu_step_pal(&mut self) {
    self.ppu_step_nstc();

    // PPU is run for 3.2 cycles on PAL
    self.ppu_pal_cycles += 1;
    if self.ppu_pal_cycles >= 5 {
      self.ppu_pal_cycles = 0;
      self.ctx.ppu().tick();
    }
  }

  pub fn irq_poll(&mut self) -> bool {
    self.mapper.poll_irq()
      || self.ctx.apu().frame_irq_flag.is_some()
      || self.ctx.apu().dmc.irq_flag.is_some()
  }

  pub fn nmi_poll(&mut self) -> bool {
    // https://www.nesdev.org/wiki/NMI
    let ppu = self.ctx.ppu();
    let res = ppu.nmi_requested.take().is_some();

    if ppu.nmi_tmp.is_some() {
      ppu.nmi_requested = ppu.nmi_tmp.take();
    }

    res
  }

  pub fn vblank_poll(&mut self) -> bool {
    self.ctx.ppu().frame_ready.take().is_some()
  }
}
