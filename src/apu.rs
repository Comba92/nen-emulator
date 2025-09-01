use blip_buf::BlipBuf;

use crate::{bus::{self, IrqFlags}, dma::Dma, emu::{Emu, Region}, utils::{byte_set_hi, byte_set_lo}};

#[derive(Default)]
pub struct DividerCounter {
  count: u16,
  pub period: u16,
}

impl DividerCounter {
  pub fn step<F: FnOnce()>(&mut self, callback: F){
    if self.count > 0 {
      self.count -= 1;
    } else {
      self.reload();
      callback();
    }
  }

  fn reload(&mut self) {
    self.count = self.period + 1;
  }
}

#[derive(Default)]
pub struct LengthCounter {
  pub count: u8,
  enabled: bool,
  halted: bool,
}

impl LengthCounter {
  const TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
  ];

  pub fn step(&mut self) {
    if !self.halted && self.count > 0 {
      self.count -= 1;
    }
  }

  fn load(&mut self, val: u8) {
    if self.enabled {
      self.count = Self::TABLE[val as usize >> 3];
    }
  }

  pub fn enable(&mut self, cond: bool) {
    self.enabled = cond;
    self.count = if cond { self.count } else { 0 };
  }
}

#[derive(Default)]
pub struct Envelope {
  start: bool,
  looping: bool,
  use_volume: bool,
  decay: u8,
  div: DividerCounter,
}

impl Envelope {
  pub fn step(&mut self) {
    // TODO: volume can be precomputed here

    self.div.step(|| {
      if self.decay > 0 {
        self.decay -= 1;
      } else if self.looping {
        self.decay = 15;
      }
    });

    if self.start {
      self.start = false;
      self.decay = 15;
      self.div.reload();
      return;
    }
  }

  fn set(&mut self, val: u8) {
    self.looping    = val & 0x20 != 0;
    self.use_volume = val & 0x10 != 0;
    self.div.period = val as u16 & 0xf;

    // TODO: volume can be precomputed here 
  }

  fn volume(&self) -> u8 {
    if self.use_volume {
      self.div.period as u8
    } else {
      self.decay
    }
  }
}

struct Sweep {
  div: DividerCounter,
  enabled: bool,
  negate: bool,
  reload: bool,
  shift: u8,
  target_period: u16,
}
impl Default for Sweep {
  fn default() -> Self {
    Self {
      div: Default::default(),
      enabled: false,
      negate: true,
      reload: false,
      shift: 0,
      target_period: 0,
    }
  }
}

// TODO: implement this for audio stuff, to have a lil cleaner interface
pub trait Channel {
  fn step_divider(&mut self);
  fn sample(&self) -> u8;
}

// https://www.nesdev.org/wiki/APU_Pulse
#[derive(Default)]
pub struct Pulse {
  div: DividerCounter,
  pub len: LengthCounter,
  pub env: Envelope,
  sweep: Sweep,
  duty_seq: u8,
  duty_cycle: u8, 
}

impl Pulse {
  const DUTIES: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 0, 0]
  ];

  pub fn write_ctrl(&mut self, val: u8) {
    self.env.set(val);
    self.len.halted = val & 0x10 != 0;
    self.duty_cycle = val >> 6;
  }

  fn write_sweep(&mut self, val: u8) {
    let sweep = &mut self.sweep;

    sweep.enabled = val & 0x80 != 0;
    sweep.div.period = ((val as u16) >> 4) & 0b111;
    sweep.negate = val & 0x8 != 0;
    sweep.shift = val & 0b111;
    sweep.reload = true;
  }

  pub fn write_timer_lo(&mut self, val: u8) {
    self.div.period = byte_set_lo(self.div.period, val);
  }

  pub fn write_timer_hi(&mut self, val: u8) {
    self.div.period = byte_set_hi(self.div.period, val & 0b111);
    self.len.load(val);

    self.duty_seq = 0;
    self.env.start = true;
  }

  pub fn step_divider(&mut self) {
    self.div.step(|| {
      self.duty_seq = (self.duty_seq + 1) % Self::DUTIES[0].len() as u8;
    });
  }

  fn is_muted(&self) -> bool {
    // Thus to fully disable the sweep unit, a program must additionally turn on the Negate flag, such as by writing $08. This ensures that the target period is not greater than the current period and therefore not greater than $7FF. 
    self.div.period < 8 || (!self.sweep.negate && self.div.period > 0x7ff)
  }

  fn step_sweep(&mut self, complement: bool) {
    // https://www.nesdev.org/wiki/APU_Sweep#Updating_the_period

    let is_muted = self.is_muted();
    let sweep = &mut self.sweep;

    sweep.div.step(|| {
      if sweep.enabled && sweep.shift > 0 && !is_muted {
        let period = self.div.period;

        let change_amt = period >> sweep.shift;
        if sweep.negate {
          // TODO: not sure about this
          sweep.target_period = period.saturating_sub(change_amt);
          sweep.target_period -= complement as u16;
        }
        else {
          sweep.target_period = period + change_amt;
        }
        self.div.period = sweep.target_period;
      }
    });

    if sweep.reload {
      sweep.div.count = sweep.div.period + 1;
      sweep.reload = false;
    }
  }

  pub fn sample(&self) -> u8 {
    if self.len.count > 0 && !self.is_muted() {
      // TODO: might be difficult, but output can be precomputed
      let seq = Self::DUTIES[self.duty_cycle as usize][self.duty_seq as usize];
      seq * self.env.volume()
    } else {
      0
    }
  }
}

// https://www.nesdev.org/wiki/APU_Triangle
#[derive(Default)]
struct Triangle {
  div: DividerCounter,
  len: LengthCounter,
  linear_count: u8,
  linear_reload: u8,
  linear_reload_flag: bool,
  sequence: u8,
}

impl Triangle {
  const TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
  ];

  fn step_divider(&mut self) {
    self.div.step(|| {
      // The sequencer is clocked by the timer as long as both the linear counter and the length counter are nonzero.
      
      if self.len.count > 0 && self.linear_count > 0 {
        self.sequence = (self.sequence + 1) % Self::TABLE.len() as u8;
      }
    });
  }

  fn linear_step(&mut self) {
    if self.linear_reload_flag {
      self.linear_count = self.linear_reload;
    } else if self.linear_count > 0 {
      self.linear_count -= 1;
    }

    if !self.len.halted {
      self.linear_reload_flag = false;
    }
  }

  fn enable(&mut self, cond: bool) {
    self.len.enable(cond);
    self.linear_count = if cond { self.linear_count } else { 0 };
  }

  fn sample(&self) -> u8 {
    // At the expense of accuracy, these can be eliminated in an emulator e.g. by halting the triangle channel when an ultrasonic frequency is set (a timer value less than 2). 
    // Other games, e.g. Zombie Nation and Bullet-Proof Software's Tetris, "silence" the triangle channel by setting the timer to $7FF, which produces a deep rumble and quiet whine. 
    
    // TODO: precompute output
    if 2 <= self.div.period && self.div.period < 0x7ff {
      Self::TABLE[self.sequence as usize]
    } else {
      0
    }
  }
}

#[derive(Default)]
struct Noise {
  div: DividerCounter,
  len: LengthCounter,
  env: Envelope,
  looping: bool,
  shift: u16,
}

impl Noise {
  const TABLE_NTSC: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160,
    202, 254, 380, 508, 762, 1016, 2034, 4068
  ];
  const TABLE_PAL: [u16; 16] = [
    4, 8, 14, 30, 60, 88, 118, 148,
    188, 236, 354, 472, 708,  944, 1890, 3778
  ];

  fn new() -> Self {
    Self {
      // On power-up, the shift register is loaded with the value 1.
      shift: 1,
      ..Default::default()
    }
  }

  fn step_divider(&mut self) {
    self.div.step(|| {
      let bit = if self.looping { 6 } else { 1 };
      let feedback = (self.shift & 1) ^ ((self.shift >> bit) & 1);
      self.shift >>= 1;
      // Bit 14, the leftmost bit, is set to the feedback calculated earlier
      self.shift |= feedback << 14;
    });
  }

  fn sample(&self) -> u8 {
    // TODO: precompute output

    if self.len.count > 0 {
      !(self.shift & 1 == 0) as u8 * self.env.volume()
    } else {
      0
    }
  }
}

#[derive(Default)]
pub struct Dmc {
  div: DividerCounter,
  irq_enabled: bool,
  looping: bool,
  output: u8,
  sample_addr: u16,
  sample_len: u16,
  
  pub dma: Dma,
  pub buffer: Option<u8>,

  shift: u8,
  bits_remaining: u8,
  silence: bool,
}

impl Dmc {
  const NTSC_RATES: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214,
    190, 160, 142, 128, 106,  84,  72,  54
  ];

  const PAL_RATES: [u16; 16] = [
    398, 354, 316, 298, 276, 236,210, 198,
    176, 148, 132, 118,  98,  78,  66,  50
  ];

  pub fn new() -> Self {
    Self {
      silence: true,
      ..Default::default()
    }
  }

  pub fn step_divider(&mut self) {
    // https://www.nesdev.org/wiki/APU_DMC#Output_unit

    self.div.step(|| {
      if !self.silence {
        if self.shift & 1 == 1 && self.output <= 125 {
          self.output += 2;
        } else if self.output >= 2 {
          self.output -= 2;
        }
        self.shift >>= 1;
      }
      
      
      if self.bits_remaining > 0 {
        self.bits_remaining -= 1;
      } else {
        self.bits_remaining = 8;

        match self.buffer.take() {
          Some(val) => {
            self.silence = false;
            self.shift = val;
          }
          None => self.silence = true,
        }
      }
    });
  }

  fn restart_sample(&mut self) {
    // When a sample is (re)started, the current address is set to the sample address, and bytes remaining is set to the sample length. 
    self.dma.load(self.sample_addr, self.sample_len);
  }

  fn enable(&mut self, cond: bool) {
    if cond {
      // If the DMC bit is set, the DMC sample will be restarted only if its bytes remaining is 0. If there are bits remaining in the 1-byte sample buffer, these will finish playing before the next sample is fetched.
      if self.dma.remaining == 0 { self.restart_sample(); }
    } else {
      // If the DMC bit is clear, the DMC bytes remaining will be set to 0 and the DMC will silence when it empties.
      self.dma.remaining = 0;
    }
  }

  pub fn sample(&mut self) -> u8 {
    self.output
  }
}

#[repr(u8)]
#[derive(Default)]
enum FrameMode {
  #[default] Step4, Step5
}

pub struct AudioBuf(pub BlipBuf);
// TODO: make sample rate configurable
impl Default for AudioBuf {
  fn default() -> Self { Self::new(&Region::default()) }
}
impl AudioBuf {
  pub fn new(region: &Region) -> Self {
    let mut blip = BlipBuf::new(48000);
    let clock_rate = match region {
      Region::NTSC => Emu::NTSC_CLOCK_RATE,
      Region::PAL => Emu::PAL_CLOCK_RATE
    };
    blip.set_rates(clock_rate as f64, 48000.0);
    Self(blip)
  }
}

#[derive(Default)]
pub struct ApuRP2A {
  p0: Pulse,
  p1: Pulse,
  tri: Triangle,
  noise: Noise,
  pub dmc: Dmc,

  frame_count: u16,
  frame_irq_disable: bool,
  frame_mode: FrameMode,

  prev_sample: f32,
  pub blip: AudioBuf,
  pub cycles: usize,
}

impl ApuRP2A {
  pub fn new(region: &Region) -> Self {
    Self {
      noise: Noise::new(),
      dmc: Dmc::new(),
      blip: AudioBuf::new(region),
      ..Default::default()
    }
  }

  fn frame_quarter_step(&mut self) {
    self.p0.env.step();
    self.p1.env.step();
    self.tri.linear_step();
    self.noise.env.step();
  }

  fn frame_half_step(&mut self) {
    self.frame_quarter_step();

    self.p0.len.step();
    self.p1.len.step();

    self.p0.step_sweep(true);
    self.p1.step_sweep(false);

    self.tri.len.step();
    self.noise.len.step();
  }
}

impl Emu {
  pub fn apu_reg_read(&mut self, addr: u16) -> u8 {
    let apu = &mut self.apu;
    match addr {
      0x4015 => {
        let mut res = 0;
        res |= ((apu.p0.len.count > 0) as u8) << 0;
        res |= ((apu.p1.len.count  > 0) as u8) << 1;
        res |= ((apu.tri.len.count > 0) as u8) << 2;
        res |= ((apu.noise.len.count > 0) as u8) << 3;
        res |= ((apu.dmc.dma.remaining > 0) as u8) << 4;
        res |= self.mem.cpu_data_bus & 0x10;
        res |= (self.mem.irq.contains(IrqFlags::FRAME) as u8) << 6;
        res |= (self.mem.irq.contains(IrqFlags::DMC) as u8) << 7;

        // TODO: If an interrupt flag was set at the same moment of the read, it will read back as 1 but it will not be cleared.
        self.mem.irq.remove(IrqFlags::FRAME);
        res
      }
      _ => self.mem.cpu_data_bus,
    }
  }

  pub fn apu_reg_write(&mut self, addr: u16, val: u8) {
    let apu = &mut self.apu;
    
    match addr {
      0x4000 => apu.p0.write_ctrl(val),
      0x4001 => apu.p0.write_sweep(val),
      0x4002 => apu.p0.write_timer_lo(val),
      0x4003 => apu.p0.write_timer_hi(val),

      0x4004 => apu.p1.write_ctrl(val),
      0x4005 => apu.p1.write_sweep(val),
      0x4006 => apu.p1.write_timer_lo(val),
      0x4007 => apu.p1.write_timer_hi(val),

      0x4008 => {
        apu.tri.len.halted = val & 0x80 != 0;
        apu.tri.linear_reload = val & 0x7f;
      }
      0x400a => apu.tri.div.period = byte_set_lo(apu.tri.div.period, val),
      0x400b => {
        apu.tri.len.load(val);
        apu.tri.div.period = byte_set_hi(apu.tri.div.period, val & 0x7);
        apu.tri.linear_reload_flag = true;
      }

      0x400c => {
        apu.noise.env.set(val);
        apu.noise.len.halted = val & 0x20 != 0;
      }
      0x400e => {
        apu.noise.looping = val & 0x80 != 0;
        self.apu.noise.div.period = match self.region() {
          Region::NTSC => Noise::TABLE_NTSC[val as usize & 0xf],
          Region::PAL => Noise::TABLE_PAL[val as usize & 0xf]
        };
      }
      0x400f => {
        apu.noise.len.load(val);
        apu.noise.env.start = true;
      }
      0x4010 => {
        apu.dmc.looping = val & 0x40 != 0;
        apu.dmc.irq_enabled = val & 0x80 != 0;

        if !apu.dmc.irq_enabled {
          self.mem.irq.remove(IrqFlags::DMC);
        }

        self.apu.dmc.div.period = match self.region() {
          Region::NTSC => Dmc::NTSC_RATES[val as usize & 0xf],
          Region::PAL => Dmc::PAL_RATES[val as usize & 0xf],
        }
      }
      0x4011 => {
        let level = val & 0x7f;

        // reduce dmc popping
        self.apu.dmc.output = if self.apu.dmc.output.abs_diff(level) <= 50 { level } else { 50 };
      }
      0x4012 => apu.dmc.sample_addr = 0xc000 + ((val as u16) * 64),
      0x4013 => apu.dmc.sample_len = ((val as u16) * 16) + 1,

      0x4015 => {
        apu.p0.len.enable (val & 0x1 != 0);
        apu.p1.len.enable (val & 0x2 != 0);
        apu.tri.enable(val & 0x4 != 0);
        apu.noise.len.enable(val & 0x8 != 0);
        apu.dmc.enable(val & 0x10 != 0);
      
        self.mem.irq.remove(IrqFlags::DMC);
      }

      0x4017 => {
        apu.frame_mode = if val & 0x80 == 0 {
          FrameMode::Step4
        } else {
          FrameMode::Step5
        };

        if val & 0x80 == 1 {
          // Writing to $4017 with bit 7 set ($80) will immediately clock all of its controlled units at the beginning of the 5-step sequence; with bit 7 clear, only the sequence is reset without clocking any of its units. 
          apu.frame_half_step();
        }

        // Interrupt inhibit flag. If set, the frame interrupt flag is cleared, otherwise it is unaffected. 
        apu.frame_irq_disable = val & 0x40 != 0;
        if apu.frame_irq_disable {
          self.mem.irq.remove(bus::IrqFlags::FRAME);
        }
        apu.frame_count = 0;

        // TODO: Writing to $4017 resets the frame counter and the quarter/half frame triggers happen simultaneously, but only on "odd" cycles (and only after the first "even" cycle after the write occurs) – thus, it happens either 2 or 3 cycles after the write (i.e. on the 2nd or 3rd cycle of the next instruction). After 2 or 3 clock cycles (depending on when the write is performed), the timer is reset. 
      }
        _ => {}
    }
  }

  pub fn read_dmc_sample(&mut self, sample: u8) {
    let dmc = &mut self.apu.dmc;
    
    dmc.buffer = Some(sample);
    dmc.bits_remaining = 8;

    dmc.dma.addr = dmc.dma.addr.wrapping_add(1);
    if dmc.dma.addr == 0 { dmc.dma.addr = 0x8000; }

    // EXTREMELY IMPORTANT to subtract this BEFORE, or else last byte won't be handled next tick
    dmc.dma.remaining -= 1;
    if dmc.dma.remaining == 0 {
      if dmc.looping {
        dmc.restart_sample();
      } else if dmc.irq_enabled {
        self.mem.irq.insert(IrqFlags::DMC);
      }
    }
  }

  pub fn apu_step(&mut self) {
    self.frame_count_step();
    let apu = &mut self.apu;
    
    // The triangle channel's timer is clocked on every CPU cycle, but the pulse, noise, and DMC timers are clocked only on every second CPU cycle and thus produce only even periods.
    apu.tri.step_divider();
    apu.dmc.step_divider();
    
    if apu.cycles % 2 == 1 {
      apu.p0.step_divider();
      apu.p1.step_divider();
      apu.noise.step_divider();
    }

    // TODO: lookup table method
    let pulse = 0.00752 * (apu.p0.sample() + apu.p1.sample()) as f32;
    let tnd = 
      0.00851 * apu.tri.sample() as f32
      + 0.00494 * apu.noise.sample() as f32
      + 0.00335 * apu.dmc.sample() as f32;

    let ext = self.mapper.sample() * 0.00568;

    let sample = (pulse + tnd + ext) * 80000.0;
    let delta = sample - apu.prev_sample;

    apu.blip.0.add_delta(apu.cycles as u32, delta as i32);
    apu.prev_sample = sample;

    apu.cycles += 1;
  }

  fn frame_count_step(&mut self) {
    // The sequencer is clocked on every other CPU cycle, so 2 CPU cycles = 1 APU cycle
    
    match self.region() {
      Region::NTSC => self.frame_count_step_ntsc(),
      Region::PAL => self.frame_count_step_pal(),
    }
  }

  fn frame_count_step_ntsc(&mut self) {
    let apu = &mut self.apu;
    match (apu.frame_count, &apu.frame_mode) {
      (3728 | 11185, _) => apu.frame_quarter_step(),
      (7456, _) => apu.frame_half_step(),
      (14914, FrameMode::Step4) => {
        if !apu.frame_irq_disable {
          self.mem.irq.insert(IrqFlags::FRAME);
        }
        
        apu.frame_count = 0;
        apu.frame_half_step();
      }
      (18640, FrameMode::Step5) => {
        apu.frame_count = 0;
        apu.frame_half_step();
      }
      _ => {}
    }

    self.apu.frame_count += 1;
  }

  fn frame_count_step_pal(&mut self) {
    let apu = &mut self.apu;
    match (apu.frame_count, &apu.frame_mode) {
      (4156 | 12469, _) => apu.frame_quarter_step(),
      (8313, _) => apu.frame_half_step(),
      (16626, FrameMode::Step4) => {
        if !apu.frame_irq_disable {
          self.mem.irq.insert(IrqFlags::FRAME);
        }
        
        apu.frame_count = 0;
        apu.frame_half_step();
      }
      (20782, FrameMode::Step5) => {
        apu.frame_count = 0;
        apu.frame_half_step();
      }
      _ => {}
    }

    self.apu.frame_count += 1;
  }
}