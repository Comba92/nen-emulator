use crate::{banks::{self, Banking, MemConfig}, cart::{CartHeader, Mirroring}, mapper::{set_byte_hi, set_byte_lo}, mem, Bus, Mapper};

#[derive(Debug, Default)]
pub struct DiskHeader {
  sides_count: Option<usize>,
  sides: Vec<DiskSide>,
}

#[derive(Default)]
pub struct DiskSide {
  raw: Vec<u8>,
  face: SideFace,
  game_name: String,
  disk_number: usize,
  boot_file_id: usize,
  files_count: usize,
  files: Vec<DiskFile>,
}
impl std::fmt::Debug for DiskSide {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("DiskSide").field("face", &self.face).field("game_name", &self.game_name).field("disk_number", &self.disk_number).field("boot_file_id", &self.boot_file_id).field("files_count", &self.files_count).field("files", &self.files).finish()
  }
}

#[derive(Default, Debug)]
enum SideFace {
  #[default] SideA, SideB,
}

#[derive(Default)]
pub struct DiskFile {
  number: usize,
  id: usize,
  name: String,
  address: u16,
  size: usize,
  kind: FileKind,
  // data: Vec<u8>,
}
impl std::fmt::Debug for DiskFile {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("DiskFile").field("number", &self.number).field("id", &self.id).field("name", &self.name).field("address", &self.address).field("size", &self.size).field("kind", &self.kind).finish()
  }
}

#[derive(Default, Debug)]
enum FileKind {
  #[default] PRAM, CRAM, VRAM
}

const FDS_MAGIC: [u8; 4] = [0x46, 0x44, 0x53, 0x1A];
pub const FDS_HEADER_SIZE: usize = 16;
const SIDE_SIZE: usize = 65500;
const SIDE_HEADER_SIZE: usize = 0x38;
const FILE_HEADER_SIZE: usize = 0x10;

fn add_gaps(buf: &mut Vec<u8>, bits_size: usize) {
  // crc value
  buf.push(0xbe);
  buf.push(0xef);

  let buf_len = buf.len();
  buf.resize(buf_len + (bits_size / 8), 0);
  // gap terminator
  buf.push(0x80);
}

impl DiskHeader {
  pub fn new(rom: &[u8]) -> Result<Self, &'static str> {
    if rom.len() < 4 {
      return Err("Not a valid FDS file");
    }

    if rom.len() < SIDE_SIZE {
      return Err("FDS file too small to contain a disk side");
    }

    let mut header = DiskHeader::default();

    let magic = &rom[..=3];
    let rom = if magic == FDS_MAGIC {
      header.sides_count = Some(rom[4] as usize);
      &rom[16..]
    } else {
      &rom
    };

    let side_len = if let Some(size) = &header.sides_count {
      *size   
    } else {
      SIDE_SIZE
    };

    if rom.len() % side_len != 0 {
      return Err("Some disk sides aren't the correct size");
    }

    let disk_sides = rom.chunks(side_len);
    dbg!(disk_sides.len());

    for disk_side in disk_sides {
      let mut raw = vec![0; 28300 / 8];
      raw.push(0x80);

      let mut side = DiskSide::default();

      // side header block
      let block1 = disk_side[0x00];
      assert_eq!(block1, 1);
      dbg!(block1);

      raw.extend_from_slice(&disk_side[..SIDE_HEADER_SIZE]);
      add_gaps(&mut raw, 976);
      
      let verify = str::from_utf8(&disk_side[0x01..0x01 + 14]).unwrap_or_default();
      dbg!(verify);

      // todo: licensee
      let licensee = disk_side[0x0f];
      dbg!(licensee);

      let game_name = str::from_utf8(&disk_side[0x10.. 0x10+3]).unwrap_or_default();
      dbg!(game_name);
      side.game_name = game_name.to_string();

      // todo: game type
      // todo: game version
      let side_number = disk_side[0x16];
      dbg!(side_number);
      side.face = match side_number {
        1 => SideFace::SideB,
        _ => SideFace::SideA,
      };

      let disk_number = disk_side[0x16];
      dbg!(disk_number);
      side.disk_number = disk_number as usize;

      // todo: disk type 1

      let boot_file = disk_side[0x19];
      dbg!(boot_file);
      side.boot_file_id = boot_file as usize;
  
      // todo: manufacturing date

      // todo: country
      let country = disk_side[0x22];
      dbg!(country);

      // todo: rewrite date

      // todo: actual  disk side
      let actual_disk_side = disk_side[0x35];
      dbg!(actual_disk_side);

      // todo: disk type 2
      // todo: disk version

      // file amount block
      let block2 = disk_side[SIDE_HEADER_SIZE];
      assert_eq!(block2, 2);
      dbg!(block2);

      raw.extend_from_slice(&disk_side[SIDE_HEADER_SIZE..SIDE_HEADER_SIZE + 0x02]);
      add_gaps(&mut raw, 976);

      let files_count = disk_side[SIDE_HEADER_SIZE + 0x01];
      dbg!(files_count);
      side.files_count = files_count as usize;

      let mut side_files = &disk_side[SIDE_HEADER_SIZE + 0x02..];
      dbg!(side_files.len());
      for _ in 0..files_count as usize {
        println!();

        let mut file = DiskFile::default();

        // file header block
        let block3 = side_files[0x00];
        assert_eq!(block3, 3);
        dbg!(block3);

        raw.extend_from_slice(&side_files[..FILE_HEADER_SIZE]);
        add_gaps(&mut raw, 976);

        file.number = side_files[0x01] as usize;
        file.id = side_files[0x02] as usize;

        let file_name = &side_files[0x03 .. 0x03 + 8];
        file.name = str::from_utf8(&file_name).unwrap_or_default().to_string();
        let file_address = &side_files[0x0b .. 0x0b + 2];
        let file_size = &side_files[0x0d .. 0x0d + 2];
        file.address = u16::from_le_bytes([file_address[0], file_address[1]]);
        file.size = u16::from_le_bytes([file_size[0], file_size[1]]) as usize;

        file.kind = match side_files[0x0f] {
          0 => FileKind::PRAM,
          1 => FileKind::CRAM,
          _ => FileKind::VRAM,
        };

        raw.extend_from_slice(&side_files[FILE_HEADER_SIZE .. FILE_HEADER_SIZE + 1 + file.size]);
        add_gaps(&mut raw, 976);

        dbg!(&file);

        // file data block
        let block4 = side_files[FILE_HEADER_SIZE];
        assert_eq!(block4, 4);
        dbg!(block4);

        // file.data = side_files[0x11 .. 0x11 + file.size].to_vec();
        side_files = &side_files[FILE_HEADER_SIZE + 1 + file.size ..];
        side.files.push(file);
      }

      side.raw = raw;
      header.sides.push(side);
    }

    println!("{:x?}", &header.sides[0].raw[..=28300 / 8 + SIDE_HEADER_SIZE + 10]);
    return Ok(header)
  }
}

#[derive(Default)]
enum DiskState {
  NotInserted,
  #[default]
  AtStart,
  Scanning,
  Rewinding,
  AtEnd
}

#[derive(Default, Debug)]
pub struct FDS {
  // Ctrl
  disk_scan_reset: bool,            // Ctrl: TxReset
  disk_motor_enabled: bool,         // Ctrl: DriveMotorCtrl -> its important for at_end
  disk_read_mode: bool,             // Ctrl: TxMode
  mirroring: Mirroring,
  disk_crc_enabled: bool,           // Ctrl: CRCEnabled -> its important for gaps
  disk_irq_enabled: bool,           // Ctrl: InterruptEnabled
  disk_irq_flag: Option<()>,
  
  disk_transferred: bool,         // Stat: ByteTx
  disk_at_end: bool,              // Stat: EndOfHead

  disk_inserted: bool,
  disk_scanning: bool,
  disk_pos: usize,
  disk_delay: usize,
  disk_in_gap: bool,

  disk_data_read: u8,
  disk_data_write: u8,

  disk_io_enable: bool,
  sound_io_enable: bool,

  timer_irq_reload: u16,
  timer_irq_count: u16,
  timer_irq_repeat: bool,
  timer_irq_enabled: bool,
  timer_irq_flag: Option<()>,

  pub disk: DiskHeader,
}

impl FDS {
  fn handle_timer_irq(&mut self) {
    if !self.timer_irq_enabled { return; }

    if self.timer_irq_count > 0 {
      self.timer_irq_count -= 1;
    } else {
      self.timer_irq_flag = Some(());
      self.timer_irq_count = self.timer_irq_reload;
      self.timer_irq_enabled = self.timer_irq_repeat;
    }
  }

  // fn disk_read(&mut self) {
  //   self.disk_data = self.disk.sides[0].raw[self.disk_offset as usize];
  //   self.disk_offset += 1;
    
  //   if self.disk_crc_enabled {
  //     self.disk_not_in_gap = true;
  //     return;
  //   }

  //   if self.disk_not_in_gap {
  //     // are we at the 0x80 byte?
  //     self.disk_not_in_gap = (self.disk_data >> 7) & 1 == 0;
  //   } else {
  //     self.disk_byte_tx = true;
  //     self.timer_irq_flag = None;
  //     self.disk_irq_flag = None;
  //   }
  // }

  // fn disk_write(&mut self) {
  //   self.disk_rw_enabled = true;
  //   self.disk_byte_tx = true;
  // }
}

impl Mapper for FDS {
  fn new(_: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.sram = Banking::new(32 * 1024, 0x6000, 32 * 1024, 1);
    banks.prg = Banking::new(8 * 1024, 0xe000, 8 * 1024, 1);

    banks.mapping.set_prg_handlers(mem::sram_read, mem::sram_write);
    banks.mapping.cpu_reads[7] = mem::prg_read;
    banks.mapping.cpu_writes[7] = |_, _, _| {}; // can't write to bios

    Box::new(Self {
      disk_inserted: true,
      disk_io_enable: true,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, _: &mut MemConfig, _: usize, _: u8) {}

  fn cart_read(&mut self, addr: usize) -> u8 {
    if !self.disk_io_enable {
      return 0xff;
    }

    match addr {
      0x4030 => {
        let mut res = 0;
        res |= self.disk_irq_flag.is_some() as u8;
        res |= (self.disk_transferred as u8) << 1;
        res |= match self.mirroring {
          Mirroring::Vertical => 0,
          _ => 1,
        } << 3;

        res |= (self.disk_at_end as u8) << 6;
        // res |= (self.disk_rw_enabled as u8) <<  7;
        res |= 1 << 7;

        self.disk_transferred = false;
        self.timer_irq_flag = None;
        self.disk_irq_flag = None;

        res
      }
      0x4031 => {
        self.disk_transferred = false;
        self.disk_irq_flag = None;
        self.disk_data_read
      }
      0x4032 => {
        let mut res = 0;
        res |= !self.disk_inserted as u8;
        res |= ((!self.disk_inserted && !self.disk_scanning) as u8) << 1;
        res |= (!self.disk_inserted as u8) <<  2;
        res |= 1 << 6;
        res
      }
      
      _ => { 0xff }
    }
  }

  fn cart_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x4020 => self.timer_irq_reload = set_byte_lo(self.timer_irq_reload, val),
      0x4021 => self.timer_irq_reload = set_byte_hi(self.timer_irq_reload, val),
      0x4022 => {
        self.timer_irq_repeat = val & 1 != 0;
        self.timer_irq_enabled = (val >> 1) & 1 != 0 && self.disk_io_enable;

        if self.timer_irq_enabled {
          self.timer_irq_count = self.timer_irq_reload;
        } else {
          self.timer_irq_flag = None;
        }
      }
      0x4023 => {
        self.disk_io_enable = val & 1 != 0;
        self.sound_io_enable = (val >> 1) & 1 != 0;

        if !self.disk_io_enable {
          self.timer_irq_enabled = false;
          self.disk_irq_enabled = false;
          self.timer_irq_flag = None;
          self.disk_irq_flag = None;
        }
      }
      0x4024 => {
        if !self.disk_io_enable { return; }
        self.disk_data_write = val;
        self.disk_transferred = false;
        self.disk_irq_flag = None;
      }
      0x4025 => {
        if !self.disk_io_enable { return; }

        self.mirroring = match (val >> 3) & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal
        };
        banks.vram.update(self.mirroring);

        // self.disk_scan_reset = val & 1 != 0;
        // self.disk_motor_enabled = (val >> 1) & 1 == 0;
        self.disk_motor_enabled = val & 1 != 0;
        self.disk_scan_reset = (val >> 1) & 1 != 0;
        self.disk_read_mode = (val >> 2) & 1 != 0;
        // self.disk_crc_tx = (val >> 4) & 1 != 0;
        self.disk_crc_enabled = (val >> 6) & 1 != 0;
        self.disk_irq_enabled = (val >> 7) & 1 != 0;

        self.disk_irq_flag = None;
      }
      _ => {}
    }
  }

  fn notify_cpu_cycle(&mut self) {
    self.handle_timer_irq();

    if !self.disk_motor_enabled {
      self.disk_at_end = true;
      self.disk_scanning = false;
      return;
    }

    if self.disk_scan_reset && !self.disk_scanning {
      return;
    }

    if self.disk_at_end {
      self.disk_delay = 50000;
      self.disk_at_end = false;
      self.disk_in_gap = true;
      self.disk_pos = 0;
      return;
    }

    if self.disk_delay > 0 {
      self.disk_delay -= 1;
      return;
    }

    if self.disk_read_mode {
      self.disk_scanning = true;

      let data = self.disk.sides[0].raw[self.disk_pos];
      let mut should_irq = true;

      if !self.disk_crc_enabled {
        // we are in a gap
        self.disk_in_gap = true;
      } else if data == 0x80 && self.disk_in_gap {
        // we reached end of a gap (found a bit set)
        self.disk_in_gap = false;
        should_irq = false;
      }
      
      if !self.disk_in_gap && should_irq {
        // we can read bytes
        self.disk_transferred = true;
        self.disk_data_read = data;
        
        if self.disk_irq_enabled {
          self.disk_irq_flag = Some(())
        }
      }
    } else {
      self.disk_transferred = true;
      if self.disk_irq_enabled {
        self.disk_irq_flag = Some(());
      }

      println!("Writing crc");
      self.disk.sides[0].raw[self.disk_pos] = 0xbb;
      self.disk_in_gap = false;
    }

    self.disk_pos += 1;
    if self.disk_pos >= self.disk.sides[0].raw.len() {
      self.disk_motor_enabled = false;
      println!("Rewinging disk");
      println!("{:#?}", self);
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.timer_irq_flag.is_some() || self.disk_irq_flag.is_some()
  }
}