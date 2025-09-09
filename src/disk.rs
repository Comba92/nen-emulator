pub struct Disk {
  pub sides: Vec<Vec<u8>>,
}

// https://github.com/SourMesen/Mesen2/blob/fabc9a62174f8734a113df6d244f5539ef6b8fcf/Core/NES/Loaders/FdsLoader.cpp#L21
// https://forums.nesdev.org/viewtopic.php?t=18668
// https://forums.nesdev.org/viewtopic.php?f=3&t=8712
impl Disk {
  const FDS_MAGIC: &[u8] = &[0x46, 0x44, 0x53, 0x1A];
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

  pub fn from(bytes: &[u8]) -> Result<Self, &'static str> {
    let (rom_start, sides_count) = if &bytes[..4] == Self::FDS_MAGIC {
      (Self::FDS_HEADER_SIZE, bytes[4] as usize)
    } else { 
      (0, bytes.len() / Self::SIDE_SIZE)
    };

    if sides_count == 0 {
      return Err("not a valid FDS rom")
    }

    println!("SIDES AMOUNT: {sides_count}");

    let mut disk = Vec::new();

    let mut img = &bytes[rom_start..];
    for _ in 0..sides_count {
      let mut side_data = Vec::with_capacity(Self::SIDE_SIZE);
      
      // Physically on the disk, there are "gaps" of 0 recorded between blocks and before the start of the disk. Length of the gaps are as follows:

      // Before the start of the disk : At least 26150 bits, 28300 typical.
      side_data.resize(28300 / 8, 0);
      side_data.push(0x80);
      
      if img[0] != 1 {
        return Err("no valid side info block")
      }
      println!("{:?}", str::from_utf8(&img[1..15]));
      println!("GAME NAME [{:?}]", str::from_utf8(&img[0x10..0x13]));

      println!("SIDE NUMBER: {}", img[0x15]);
      println!("DISK NUMBER: {}", img[0x16]);

      if !img[0x1a..0x1a+5].iter().all(|x| *x == 0xff) {
        return Err("no 0xff chunk at offset 0x1a in side info block")
      }

      // disk info block is 0x38 (56) bytes
      side_data.extend_from_slice(&img[..0x38]);
      // Self::push_gaps_and_data(&mut side_data, &img[..0x38]);
      side_data.push(0xde);
      side_data.push(0xad);

      if img[0x38] != 2 {
        return Err("no valid file amount block")
      }
      println!("FILES AMOUNT: {}", img[0x39]);

      let files_count = img[0x39];
      // file info block is 2 bytes
      Self::push_gaps_and_data(&mut side_data, &img[0x38..0x3a]);

      let mut file = &img[0x3a..];
      println!();

      let mut parsed_bytes = 0x3a;
      for i in 0..files_count {
        println!("FILE {i}");

        assert_eq!(file[0], 3);
        if file[0] != 3 {
          return Err("no valid file header block");
        }
        println!("FILE NUMBER: {}", file[1]);
        println!("FILE ID: {}", file[2]);
        println!("FILE NAME: {:?}", str::from_utf8(&file[0x3..0xb]));
        println!("FILE TYPE: {}", file[0x0f]);
        
        let file_size = u16::from_le_bytes([file[0x0d], file[0x0e]]) as usize;
        println!("FILE SIZE: {}", file_size);

        // file header block is 0x10 (16) bytes
        Self::push_gaps_and_data(&mut side_data, &file[..0x10]);

        assert_eq!(file[0x10], 4);
        if file[0x10] != 4 {
          return Err("no valid file data block");
        }

        parsed_bytes += 0x11 + file_size;
        // TODO: handle case when we go over 65500 bytes
        Self::push_gaps_and_data(&mut side_data, &file[0x11..0x11 + file_size]);
        file = &file[0x11 + file_size..];
        println!()
      }

      // After the last file block, fill a side with all 0 so that the disk side reaches exactly 65500 bytes. 
      img = &img[Self::SIDE_SIZE..];
      println!("Final side size: {}", side_data.len());
      if side_data.len() < Self::SIDE_SIZE {
        side_data.resize(Self::SIDE_SIZE, 0);
      }
      disk.push(side_data);
    }

    Ok(Self { sides: disk })
  }
}