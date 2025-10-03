pub struct Disk {
  pub sides_bytes: Vec<Vec<u8>>,
  pub sides_data: Vec<SideData>,
}

#[derive(Default, Debug)]
pub enum Side { #[default] SideA, SideB }

#[derive(Default, Debug)]
pub struct SideData {
  title: String,
  disk_side: Side,
  disk_number: u8,
  files_count: u8,
  files: Vec<FileData>,
}

#[derive(Default, Debug)]
pub enum FileKind { #[default] PRAM, CRAM, VRAM }

#[derive(Default, Debug)]
pub struct FileData {
  number: u8,
  id: u8,
  name: String,
  address: u16,
  size: u16,
  kind: FileKind
}

// https://github.com/SourMesen/Mesen2/blob/fabc9a62174f8734a113df6d244f5539ef6b8fcf/Core/NES/Loaders/FdsLoader.cpp#L21
// https://github.com/ares-emulator/ares/blob/0b2a85f80321aca7af9df37555edfdd5c4d22a9c/mia/medium/famicom-disk-system.cpp
// https://forums.nesdev.org/viewtopic.php?t=18668
// https://forums.nesdev.org/viewtopic.php?f=3&t=8712
impl Disk {
  const FDS_MAGIC: &[u8] = &[0x46, 0x44, 0x53, 0x1A];
  const FDS_NINTENDO_STR: &[u8] = "*NINTENDO-HVC*".as_bytes();
  const FDS_HEADER_SIZE: usize = 16;
  const SIDE_SIZE: usize = 65500;

  fn push_gaps_and_data(data: &mut Vec<u8>, block: &[u8]) {
    // Gap between blocks : At least 480 bits, 976 bits typical.
    data.extend(std::iter::repeat_n(0, 976/8));
    // Gaps are teminated by a single '1' bit. In terms of bytes, it would be $80, as the data is stored in little endian format. 
    data.push(0x80);
    
    data.extend_from_slice(block);
    // At the end of each block, a 16-bit CRC is stored.
    // fake CRC value
    data.push(0xde);
    data.push(0xad);
  }

  pub fn is_valid_fds(bytes: &[u8]) -> bool {
    let (rom_start, sides_count) = if &bytes[..4] == Self::FDS_MAGIC {
      (Self::FDS_HEADER_SIZE, bytes[4] as usize)
    } else { 
      (0, bytes.len() / Self::SIDE_SIZE)
    };

    if sides_count == 0 { return false; }

    // we only check for the first side nintendo bytes
    bytes[rom_start] == 1 && &bytes[rom_start+1..rom_start+15] == Self::FDS_NINTENDO_STR
  }

  // TODO: clean up
  pub fn from(bytes: &[u8]) -> Result<Self, &'static str> {
    let (rom_start, sides_count) = if &bytes[..4] == Self::FDS_MAGIC {
      (Self::FDS_HEADER_SIZE, bytes[4] as usize)
    } else { 
      (0, bytes.len() / Self::SIDE_SIZE)
    };

    if sides_count == 0 {
      return Err("not a valid FDS rom")
    }

    let mut sides_bytes = Vec::new();
    let mut sides_data = Vec::new();

    let mut img = &bytes[rom_start..];
    for _ in 0..sides_count {
      let mut side_bytes = Vec::with_capacity(Self::SIDE_SIZE);
      
      // Physically on the disk, there are "gaps" of 0 recorded between blocks and before the start of the disk. Length of the gaps are as follows:
      // Before the start of the disk : At least 26150 bits, 28300 typical.
      side_bytes.resize(28300 / 8, 0);
      side_bytes.push(0x80);
      
      if img[0] != 1 {
        return Err("no valid side info block")
      }
      if &img[1..15] != Self::FDS_NINTENDO_STR {
        return Err("not a valid FDS rom");
      }

      let mut side_data = SideData::default();
      side_data.title = String::from_utf8_lossy(&img[0x10..0x13]).into_owned();
      side_data.disk_side = if img[0x15] == 0 { Side::SideA } else { Side::SideB };
      side_data.disk_number = img[0x16];

      // disk info block is 0x38 (56) bytes
      side_bytes.extend_from_slice(&img[..0x38]);
      side_bytes.push(0xde);
      side_bytes.push(0xad);

      if img[0x38] != 2 {
        return Err("no valid file amount block")
      }

      let files_count = img[0x39];
      side_data.files_count = files_count;
      
      // file info block is 2 bytes
      Self::push_gaps_and_data(&mut side_bytes, &img[0x38..0x3a]);

      let mut file = &img[0x3a..];
      let mut parsed_bytes = 0x3a;
      for _ in 0..files_count {
        // if no more files are found, simply stop and fill rest of disk with zeroes
        if file[0] != 3 { break; }

        let mut file_data = FileData::default();
        
        file_data.number = file[1];
        file_data.id = file[2];
        file_data.name = String::from_utf8_lossy(&file[0x3..0xb]).into_owned()
          .trim_end_matches(|c: char| c.is_control())
          .to_string();

        file_data.address = u16::from_le_bytes([file[0xb], file[0xc]]);
        file_data.size = u16::from_le_bytes([file[0xd], file[0xe]]);
        file_data.kind = match file[0xf] {
          0 => FileKind::PRAM,
          1 => FileKind::CRAM,
          _ => FileKind::VRAM,
        };

        let file_size = file_data.size as usize;

        // file header block is 0x10 (16) bytes
        Self::push_gaps_and_data(&mut side_bytes, &file[..0x10]);

        if file[0x10] != 4 { break; }

        Self::push_gaps_and_data(&mut side_bytes, &file[0x10..0x10 + file_size + 1]);
        
        file = &file[0x10 + file_size + 1..];
        // TODO: handle case when we go over 65500 bytes
        parsed_bytes += 0x10 + file_size + 1;

        side_data.files.push(file_data);
      }

      // After the last file block, fill a side with all 0 so that the disk side reaches exactly 65500 bytes. 
      img = &img[Self::SIDE_SIZE..];
      if side_bytes.len() < Self::SIDE_SIZE {
        side_bytes.resize(Self::SIDE_SIZE, 0);
      }

      sides_bytes.push(side_bytes);
      sides_data.push(side_data);
    }

    println!("==[DISK READY]==\n{:?}", sides_data);
    Ok(Self { sides_bytes, sides_data })
  }
}