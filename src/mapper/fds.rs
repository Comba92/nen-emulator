use crate::{bus::{Bus, IrqFlags}, emu::Mirroring, mapper::Mapper, utils::{byte_set_hi, byte_set_lo}};

// https://www.nesdev.org/wiki/Family_Computer_Disk_System
// https://www.nesdev.org/wiki/FDS_RAM_adaptor_cable_pinout
// https://forums.nesdev.org/viewtopic.php?p=91528 
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FDS {
  pub disks: Vec<Vec<u8>>,
  pub disk_inserted: bool,
  disk_select: usize,
  head: usize,
  spin_delay: usize,
  eject_delay: usize,

  data_buf: u8,
  disk_irq_pending: bool,

  timer_count: u16,
  timer_reload: u16,
  timer_repeat: bool,
  timer_enabled: bool,

  disk_enabled: bool,
  audio_enabled: bool,

  disk_reset: bool,
  motor_enabled: bool,
  read_mode: bool,
  mirroring: bool,
  crc_ctrl: bool,
  crc_enabled: bool,
  disk_irq_enabled: bool,

  disk_at_end: bool,
  disk_spinning: bool,
  disk_in_gap: bool,

  audio: fds::Audio,
}
impl FDS {
  fn disk_read(&self) -> u8 {
    self.disks[self.disk_select][self.head]
  }

  fn disk_write(&mut self, val: u8) {
    self.disks[self.disk_select][self.head] = val;
  }

  // fn update_crc(&mut self, val: u8) {
  //   self.crc_acc ^= val as u16;
  //   for _ in 0..8 {
  //     let carry = self.crc_acc & 1;
  //     self.crc_acc >>= 1;
  //     self.crc_acc ^= 0x8408 * carry;
  //   }
  // }
}

// https://github.com/SourMesen/Mesen2/tree/master/Core/NES/Mappers/FDS
mod fds {
  use crate::utils::{byte_set_hi, byte_set_lo};

  #[derive(Default)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  pub struct Env {
    enabled: bool,
    speed: u8,
    direction: bool,
    pub freq: u16,
    pub volume_gain: u8,
    timer: u16,
    pub master_speed: u8,
  }
  impl Env {
    // 0x4082 / 0x4084
    pub fn write_freq_lo(&mut self, val: u8) {
      self.freq = byte_set_lo(self.freq, val);
    }

    // 0x4080 // 0x4085
    pub fn write_ctrl(&mut self, val: u8) {
      self.enabled = val & 0x80 == 0;
      self.direction = val & 0x40 > 0;
      
      self.speed = val & 0x3f;
      self.reset_timer();
      if !self.enabled { self.volume_gain = self.speed; }
    }

    fn step(&mut self) {
      if !self.enabled || self.master_speed == 0 { return; }

      if self.timer > 0 {
        self.timer -= 1;
      } else {
        self.reset_timer();

        match self.direction {
          true  => self.volume_gain = (self.volume_gain + 1).min(31),
          false => self.volume_gain = self.volume_gain.saturating_sub(1),
        }
      }
    }

    pub fn reset_timer(&mut self) {
      self.timer = 8 * (self.speed as u16 + 1) * (self.master_speed as u16 + 1);
    }
  }

  #[derive(Default)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  pub struct Mod {
    pub env: Env,
    pub count: i8,
    halted: bool,
    tbl: Table,
    pos: u8,
    acc: u16,
    output: i32,
  }
  impl Mod {
    const TABLE: [i8; 8] = [0, 1, 2, 4, -128, -4, -2, -1];

    // 0x4085
    pub fn write_count(&mut self, val: u8) {
      // (7-bit signed; minimum $40; maximum $3F)
      // The mod counter is a signed 7-bit value, and will wrap if overflowed, i.e. 63 + 1 = -64 after wrap, and -64 - 1 = 63. 
      let val = val as i16;
      self.count = if val >= 64 {
        val - 128
      } else if val < -64 {
        val + 128
      } else {
        val
      } as i8;
    }

    // 0x4087
    pub fn write_ctrl(&mut self, val: u8) {
      self.env.freq = byte_set_hi(self.env.freq, val & 0xf);
      self.env.reset_timer();
      
      self.halted = val & 0x80 > 0;
      if self.halted {
        self.acc = 0;
      }
    }

    // 0x4088
    pub fn write_table(&mut self, val: u8) {
      if self.halted {
        self.tbl.0[(self.pos as usize) % 64] = val & 0x7;
        self.tbl.0[(self.pos as usize + 1) % 64] = val & 0x7;
        self.pos = (self.pos + 2) % 64;
      }
    }

    fn step(&mut self) {
      if self.halted { return; }

      self.acc += self.env.freq;
      if self.acc < self.env.freq {
        let val = self.tbl.0[self.pos as usize];
        let adj = Self::TABLE[val as usize];

        let count = if adj == -128 { 0 } else { self.count + adj };
        self.write_count(count as u8);
        self.pos = (self.pos + 1) % 64;
      }
    }

    // https://www.nesdev.org/wiki/FDS_audio#Modulation_unit
    pub fn update(&mut self, pitch: u16) {
      // 1. multiply counter by gain
      let mut temp = self.count as i32 * self.env.volume_gain as i32;
            
      let mut remainder = temp & 0xf;
      temp >>= 4;

      if remainder > 0 && temp & 0x80 == 0 {
        temp += if self.count < 0 { -1 } else { 2 };
      }

      if temp >= 192 {
        temp -= 256;
      } else if temp < -64 {
        temp += 256;
      }

      temp = pitch as i32 * temp;
      remainder = temp & 0x3f;
      temp >>= 6;
      if remainder >= 32 {
        temp += 1;
      }

      self.output = temp;
      // 2. round up to 6 bits only if sign positive (ignoring bit 4)
      // if tmp & 0x0f > 0 && tmp & 0x800 == 0 { tmp += 0x20; }

      // 3. drop 4 bits and center to 0x40
      // tmp += 0x400;
      // tmp = (tmp >> 4) & 0xff;

      // 4. multiply by pitch to get the 20-bit unsigned result
      // (self.env.freq as i32 * tmp) & 0xf_ffff
    }
  }

  #[derive(Default)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  pub struct Wave {
    ram: Table,
    pub ram_writable: bool,
    pos: u8,
    acc: i32,
    halted: bool,
  }

  #[cfg(feature = "serde")]
use serde_big_array::BigArray;
  
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  pub struct Table(#[cfg_attr(feature = "serde", serde(with = "BigArray"))] pub [u8; 64]);
  impl Default for Table { fn default() -> Self { Self([0; 64]) } }

  #[derive(Default)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  pub struct Audio {
    env_halted: bool,
    pub master_volume: u8,

    pub env: Env,
    pub modl: Mod,
    pub wave: Wave,
  }
  impl Audio {
    const VOLUMES: [u8; 4] = [36, 24, 17, 14];

    // 0x4040
    pub fn ram_read(&self, addr: u16) -> u8 {
      let pos = if self.wave.ram_writable {
        (addr as usize - 0x4040) % 64
      } else {
        self.wave.pos as usize
      };

      self.wave.ram.0[pos] | 0x40
    }

    // 0x4040
    pub fn ram_write(&mut self, addr: u16, val: u8) {
      if self.wave.ram_writable {
        self.wave.ram.0[addr as usize - 0x4040] = val & 0x3f;
      }
    }

    // 0x4083
    pub fn write_ctrl(&mut self, val: u8) {
      self.env.freq = byte_set_hi(self.env.freq, val & 0xf);
      self.modl.update(self.env.freq);
      
      self.env_halted = val & 0x40 > 0;


      // Bit 6 halts just the envelopes without halting the waveform, and also resets both of their timers. 
      if self.env_halted {
        self.env.reset_timer();
        self.modl.env.reset_timer();
      }

      // The high bit of this register halts the waveform and resets its phase to 0. Note that if halted it will output the constant value at $4040, and writes to the volume register $4080 or master volume $4089 will affect the output.
      // The envelopes are not ticked while the waveform is halted. 
      self.wave.halted = val & 0x80 > 0;
      if self.wave.halted {
        self.wave.pos = 0;
      }
    }

    pub fn step(&mut self, _cycles: usize) {
      if !self.wave.halted && !self.env_halted {
        self.env.step();
        self.modl.env.step();
        self.modl.update(self.env.freq);
      }

      self.modl.step();
      self.modl.update(self.env.freq);

      let sum = self.env.freq as i32 + self.modl.output;
      if !self.wave.halted && sum > 0 {
        self.wave.acc += sum;
        if self.wave.acc < sum {
          self.wave.pos = (self.wave.pos + 1) % 64; 
        }
      }
    }
    
    pub fn sample(&self) -> f64 {
      let level = self.env.volume_gain.min(32) as i32 * Self::VOLUMES[self.master_volume as usize] as i32; // max level: 1152
      let output = (self.wave.ram.0[self.wave.pos as usize] as i32 * level) as f64 / 1152.0;
      output as f64
    }
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for FDS {
  fn new(_: &mut Bus) -> Box<Self> {
    // everything else is initialized in the bus constructor
    Box::new(Self {
      disk_inserted: true,
      ..Default::default()
    })
  }

  fn cart_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    match addr {
      0x4030 => {
        let mut res = 0;
        res |= mem.irq.contains(IrqFlags::MAPPER) as u8;
        res |= (self.mirroring as u8) << 3;
        // bit 4 is set if crc check fails
        res |= (self.disk_at_end as u8) << 6;
        res |= (self.disk_irq_pending as u8) << 7;
        
        self.disk_irq_pending = false;
        mem.irq.remove(IrqFlags::MAPPER);
        mem.irq.remove(IrqFlags::DISK);

        res
      }
      0x4031 => {
        self.disk_irq_pending = false;
        mem.irq.remove(IrqFlags::DISK);
        self.data_buf
      }
      0x4032 => {
        let mut res = 0;
        res |= !self.disk_inserted as u8;
        res |= ((!self.disk_inserted || !self.disk_spinning) as u8) << 1;
        res |= (!self.disk_inserted as u8) << 2;

        mem.irq.remove(IrqFlags::DISK);

        res | (mem.cpu_data_bus & 0xf8)
      }

      0x4040..=0x407f => self.audio.ram_read(addr),

      0x4090 => self.audio.env.volume_gain & 0x3f | 0x40,
      // 0x4091 => todo!("wave acc read"),
      0x4092 => self.audio.modl.env.volume_gain & 0x3f | 0x40,
      // 0x4093 => todo!("mod table addr acc"),
      // 0x4094 => todo!("mod counter gain res"),
      // 0x4095 => todo!("mod counter incr"),
      // 0x4096 => todo!("wavetable value"),
      // 0x4097 => todo!("mod counter value"),
      _ => 0xff
    }
  }

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match addr {
      0x4020 => self.timer_reload = byte_set_lo(self.timer_reload, val),
      0x4021 => self.timer_reload = byte_set_hi(self.timer_reload, val),
    
      0x4022 => {
        self.timer_repeat = val & 0x1 > 0;
        self.timer_enabled = val & 0x2 > 0 && self.disk_enabled;

        if self.timer_enabled {
          self.timer_count = self.timer_reload;
        } else {
          mem.irq.remove(IrqFlags::MAPPER);
        }
      }

      0x4023 => {
        self.disk_enabled = val & 0x1 > 0;
        self.audio_enabled = val & 0x2 > 0;

        // Clearing $4023.0 will immediately stop the IRQ counter and acknowledge any pending timer IRQs.
        if !self.disk_enabled {
          self.timer_enabled = false;
          mem.irq.remove(IrqFlags::MAPPER);
          mem.irq.remove(IrqFlags::DISK);
        }
      }

      0x4024 => if self.disk_enabled {
        self.data_buf = val as u8;
        self.disk_irq_pending = false;
        mem.irq.remove(IrqFlags::DISK);
      }

      0x4025 => if self.disk_enabled {
        // the falling edge of this signal would instruct the drive to stop its motor (and therefore end the current scan of the disk)
        self.motor_enabled = val & 0x1 > 0;

        // while high, this instructs the storage media pointer to be reset (and stay reset) at the beginning of the media
        // while low, the media pointer is to be advanced at a constant rate, and data progressively transferred to/from the media
        self.disk_reset = val & 0x2 > 0;
        // while low, this signal indicates that data appearing on the "write data" signal pin is to be written to the storage media.
        self.read_mode = val & 0x4 > 0;

        let mirroring = if val & 0x8 > 0 {
          Mirroring::Horizontal
        } else {
          Mirroring::Vertical
        };
        mem.banks.vram.mirror(&mirroring);
        self.mirroring = val & 0x8 > 0;

        // ROM BIOS subroutines set this bit while processing the CRC data at the end of a block.
        self.crc_ctrl = val & 0x10 > 0;

        // This bit is typically set while the disk head is in a GAP period on the disk.
        self.crc_enabled = val & 0x40 > 0;
        self.disk_irq_enabled = val & 0x80 > 0;

        mem.irq.remove(IrqFlags::DISK);
      }

      0x4040..=0x407f => self.audio.ram_write(addr, val),

      // TODO: disable registers if audio is disabled
      0x4080 => self.audio.env.write_ctrl(val),
      0x4082 => {
        self.audio.env.write_freq_lo(val);
        self.audio.modl.update(self.audio.env.freq);
      }

      0x4083 => self.audio.write_ctrl(val),

      0x4084 => {
        self.audio.modl.env.write_ctrl(val);
        self.audio.modl.update(self.audio.env.freq);
      }
      0x4085 => {
        self.audio.modl.write_count(val);
        self.audio.modl.update(self.audio.env.freq);
      }
      0x4086 => self.audio.modl.env.write_freq_lo(val),
      0x4087 => self.audio.modl.write_ctrl(val),
      0x4088 => self.audio.modl.write_table(val),

      0x4089 => {
        self.audio.master_volume = val & 0x3;
        self.audio.wave.ram_writable = val & 0x80 > 0;
      }
      0x408a => {
        self.audio.env.master_speed = val;
        self.audio.modl.env.master_speed = val;
      }
      
      _ => {}
    }
  }

  fn prg_write(&mut self, _mem: &mut Bus, _addr: u16, _val: u8) {}

  // https://forums.nesdev.org/viewtopic.php?p=91528#p91528
  fn step(&mut self, mem: &mut Bus, cycles: usize) {
    if self.timer_enabled {
      if self.timer_count > 0 {
        self.timer_count -= 1;
      } else {
        mem.irq.insert(IrqFlags::MAPPER);
        self.timer_count = self.timer_reload;
        self.timer_enabled = self.timer_repeat;
      }
    }

    if self.audio_enabled {
      self.audio.step(cycles);
    }

    if self.eject_delay > 0 {
      self.eject_delay -= 1;
      if self.eject_delay == 0 {
        self.disk_inserted = true;
      }
    }

    // Motor is stopped, head should stay at end
    if !self.disk_inserted || !self.motor_enabled {
      self.disk_at_end = true;
      self.disk_spinning = false;
      return;
    }

    // Head should stay at start, unless it is already spinning (in that case disk_reset is ignored)
    if self.disk_reset && !self.disk_spinning { return; }

    // Head is at end, rewind disk with delay. also we should set disk_in_gap, as disk starts with a gap
    if self.disk_at_end {
      self.spin_delay = 50_000;
      self.disk_at_end = false;
      self.head = 0;
      self.disk_in_gap = true;
    }

    if self.spin_delay > 0 {
      self.spin_delay -= 1;
      return;
    }

    self.disk_spinning = true;

    if self.read_mode {
      let data = self.disk_read();

      // During reads, setting this bit instructs the 2C33 to wait for the first set bit (block start mark) to be read off the disk,
      // before accumulating any serial data in the FDS's internal shift registers, and setting the byte transfer ready flag for the first time (and then every 8-bit subsequent transfer afterwards).
      if !self.crc_enabled {
        self.disk_in_gap = true;
      } else if self.disk_in_gap && data > 0 {
        // if we are in a gap and we find a nonzero value, we reached the end of a gap
        self.disk_in_gap = false;
      } else if !self.disk_in_gap {
        // we are in data section
        self.data_buf = data;
        if self.disk_irq_enabled {
          mem.irq.insert(IrqFlags::DISK);
        }
      }
    } else {
      let mut data = 0;

      if !self.crc_ctrl {
        data = self.data_buf;
        if self.disk_irq_enabled {
          mem.irq.insert(IrqFlags::DISK);
        }
      }

      // During writes, setting this bit instructs the 2C33 to immediately load the contents of $4024 into a shift register, 
      // set the byte transfer flag, start writing the data from the shift register onto the disk, and repeat this process on subsequent 8-bit transfers.
      // While this bit is 0, data in $4024 is ignored, and a stream of 0's is written to the disk instead.
      if !self.crc_enabled {
        // we are in a gap, don't write anything
        data = 0;
      } else if self.crc_ctrl {
        // fake crc
        data = 0x69;
      }

      self.disk_write(data);
      self.disk_in_gap = true;
    }

    self.head += 1;
    if self.head >= self.disks[self.disk_select as usize].len() {
      // stop motor, so that it gets rewinded
      self.motor_enabled = false;
      if self.disk_irq_enabled {
        mem.irq.insert(IrqFlags::DISK);
      }
    } else {
      // we read a byte from disk, set a delay so that cpu has time to handle the IRQ before fetching a new byte from disk
      self.spin_delay = 149;
    }
  }

  fn notify_cpu_addr(&mut self, _mem: &mut Bus, addr: u16, _val: Option<u8>) {
    // eventual bios hooks here
    match addr {
      0xe1f8 => {
        println!("[BIOS: ${addr:04x}] LoadFiles()");
      }
      0xe237 => {
        println!("[BIOS: ${addr:04x}] AppendFile()");
      }
      0xe239 => {
        println!("[BIOS: ${addr:04x}] WriteFile()");
      }
      0xe2b7 => {
        println!("[BIOS: ${addr:04x}] CheckFileCount()");
      }
      0xe2bb => {
        println!("[BIOS: ${addr:04x}] AdjustFileCount()");
      }
      0xe301 => {
        println!("[BIOS: ${addr:04x}] SetFileCount1()");
      }
      0xe305 => {
        println!("[BIOS: ${addr:04x}] SetFileCount()");
      }
      0xe32a => {
        println!("[BIOS: ${addr:04x}] GetDiskInfo()");
      }
      0xe445 => {
        // TODO: automatic side picker
        println!("[BIOS: ${addr:04x}] CheckDiskHeader()");
      }

      _ => {}
    }
  }

  fn special_input(&mut self) {
    self.disk_select = (self.disk_select + 1) % self.disks.len();
    // the old disk is ejected. set a delay before inserting the new one
    self.disk_inserted = false;
    // this delay works well
    self.eject_delay = 1_000_000;

    println!("Current disk selected: {:?}", self.disk_select);
  }

  fn sample(&self) -> f64 {
    // self.audio.sample()
    0.0 
  }
}