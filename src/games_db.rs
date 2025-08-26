use std::{collections::HashMap, io::Read, sync::LazyLock};
use crate::{cart::{ CartHeader, HeaderFormat}, emu::{Mirroring, Region}};

#[derive(Debug, Default, bitcode::Encode, bitcode::Decode)]
pub struct GameData {
  pub title: String,

  pub rom_total_size: usize,
  pub rom_crc32: u32,
  // pub rom_sha1: String,
  // pub rom_sum16: String,
  // pub rom_sha1: [u8; 20],

  pub prg_size: usize,
  pub prg_crc32: u32,
  // pub prg_sha1: String,
  // pub prg_sum16: String,
  // pub prg_sha1: [u8; 20],
  
  pub chr_size: usize,
  pub prgram_size: usize,
  pub prgnvram_size: usize,
  pub chrram_size: usize,
  // no game has chrnvram
  // pub chrnvram_size: usize,

  pub mapper: u16,
  pub submapper: u8,
  pub mirroring: Mirroring,
  pub region: Region,
  pub has_battery: bool,

  pub console: u8,
  pub expansions: u8,
}
impl From<&GameData> for CartHeader {
  fn from(value: &GameData) -> Self {
    let chr_size = if value.chr_size > 0 {
      value.chr_size
    } else if value.chrram_size > 0 {
      value.chrram_size
    } else {
      8 * 1024
    };

    let wram_size = if value.prgram_size > 0 {
      value.prgram_size
    } else if value.prgnvram_size > 0 {
      value.prgnvram_size
    } else {
      0
    };
    
    Self {
      prg_size: value.prg_size,
      chr_size,
      wram_size,
      has_chr_ram: value.chrram_size > 0,
      has_prg_ram: value.prg_size > 0 || value.prgnvram_size > 0,
      
      mirroring: value.mirroring.clone(),
      region: value.region.clone(),
      mapper: value.mapper,
      submapper: value.submapper,
      has_battery: value.has_battery,

      // if we made a header here, it means it was headerless
      format: HeaderFormat::Headerless,
      
      ..Default::default()
    }
  }
}

pub struct GamesDB {
  games: Vec<GameData>,
  rom_map: HashMap<u32, usize>,
  prg_map: HashMap<u32, usize>,
}
impl GamesDB {
  pub fn new(games: Vec<GameData>) -> Self {
    let rom_map = games.iter()
      .enumerate()
      // .map(|(i, e)| (e.rom_sha1.clone(), i))
      .map(|(i, e)| (e.rom_crc32, i))
      .collect::<HashMap<_, _>>();

    let prg_map = games.iter()
      .enumerate()
      // .map(|(i, e)| (e.prg_sha1.clone(), i))
      .map(|(i, e)| (e.prg_crc32, i))
      .collect::<HashMap<_, _>>();

    Self {
      games,
      rom_map,
      prg_map,
    }
  }

  pub fn query(&self, rom: &[u8]) -> Option<&GameData> {
    // let mut sha1 = sha1_smol::Sha1::new();
    // sha1.update(&rom[16..]);
    // let rom_hash = sha1.digest().bytes();

    let rom_hash = crc32fast::hash(&rom);

    let index  = self.rom_map.get(&rom_hash)
    .or_else(move || {
      // rom hash not found, try parsing the header and hash prg
      let header = CartHeader::parse(rom);
      let prg_size = header.map_or(None, |x| Some(x.prg_size));

      match prg_size {
        Some(prg_size) => {
          // sha1.reset();
          // sha1.update(&rom[16..16+prg_size]);
          // let prg_hash = sha1.digest().bytes();

          let prg_hash = crc32fast::hash(&rom[..prg_size]);
          self.prg_map.get(&prg_hash)
        }

        None => None,
      }
    });

    index.and_then(|i| Some(&self.games[*i]))
  }
}

// https://forums.nesdev.org/viewtopic.php?t=19940
pub static GAMES_DB: LazyLock<GamesDB>  = LazyLock::new(|| {
  let db = include_bytes!("../utils/nes20db.bitcode.gzip").as_slice();
  
  let mut encoded = std::io::BufReader::new(flate2::read::GzDecoder::new(db));
  let mut buf = Vec::new();
  encoded.read_to_end(&mut buf).unwrap();
  let games: Vec<GameData> = bitcode::decode(&buf).unwrap();

  GamesDB::new(games)
});


#[cfg(test)]
mod tests {
use std::{collections::BTreeSet, io::Read};
use crate::games_db::{GameData, GAMES_DB};

#[test]
fn db_access_test() {
  println!("Number of entries: {}", GAMES_DB.games.len());

  let rom = include_bytes!("../roms/metroid.nes");
  let res = GAMES_DB.query(&rom[16..]);

  println!("{res:?}");
}

#[test]
fn decode_test() {
  let file = include_bytes!("../utils/nes20db.bitcode.gzip").as_slice();

  let mut decode = flate2::read::GzDecoder::new(file);
  let mut buf = Vec::new();
  decode.read_to_end(&mut buf).unwrap();

  let _: Vec<GameData> = bitcode::decode(&buf).unwrap();
}

  #[test]
fn count_prgram() {
  let both = GAMES_DB.games.iter().filter(|x| x.prgram_size > 0 && x.prgnvram_size > 0).count();
  let only_prgram = GAMES_DB.games.iter().filter(|x| x.prgram_size > 0).count();
  let only_prgnvram = GAMES_DB.games.iter().filter(|x| x.prgnvram_size > 0).count();

  dbg!(both);
  dbg!(only_prgram);
  dbg!(only_prgnvram);

  let games_with_both = GAMES_DB.games.iter()
    .filter(|x| x.prgram_size > 0 && x.prgnvram_size > 0)
    .map(|x| x.mapper)
    .collect::<BTreeSet<_>>();
  println!("{:#?}", games_with_both);
}

#[test]
fn count_chr() {
  let both = GAMES_DB.games.iter().filter(|x| x.chr_size > 0 && x.chrram_size > 0).count();
  let only_chr = GAMES_DB.games.iter().filter(|x| x.chr_size > 0).count();
  let only_chrram = GAMES_DB.games.iter().filter(|x| x.chrram_size > 0).count();

  dbg!(both);
  dbg!(only_chr);
  dbg!(only_chrram);

  let games_with_both = GAMES_DB.games.iter()
    .filter(|x| x.chr_size > 0 && x.chrram_size > 0)
    .map(|x| x.mapper)
    .collect::<BTreeSet<_>>();
  println!("{:#?}", games_with_both);
}
}