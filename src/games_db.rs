use std::{collections::HashMap, io::Read, sync::LazyLock};
use sha1::Digest;

use crate::emu::{Mirroring, Region};

#[derive(Debug, Default, bitcode::Encode, bitcode::Decode)]
pub struct GameData {
  pub title: String,

  pub rom_total_size: usize,
  pub crc32: String,
  pub sha1: String,
  pub sum16: String,

  pub prg_size: usize,
  pub chr_size: Option<usize>,
  pub prgram_size: Option<usize>,
  pub prgnvram_size: Option<usize>,
  pub chrram_size: Option<usize>,
  pub chrnvram_size: Option<usize>,

  pub mapper: usize,
  pub submapper: usize,
  pub mirroring: Mirroring,
  pub battery: bool,
  pub region: Region,

  pub console: u8,
  pub expansions: u8,
}

pub static GAMES_DB: LazyLock<HashMap<String, GameData>>  = LazyLock::new(|| {
  let db = include_bytes!("../utils/nes_games_db.bitcode.gzip").as_slice();
  
  let mut encoded = std::io::BufReader::new(flate2::read::GzDecoder::new(db));
  let mut buf = Vec::new();
  encoded.read_to_end(&mut buf).unwrap();

  let entries: Vec<GameData> = bitcode::decode(&buf).unwrap();

  let map = entries.into_iter()
    .map(|e| (e.sha1.clone().to_lowercase(), e))
    .collect::<HashMap<_, _>>();

  map
});

#[test]
fn db_access_test() {
  println!("Number of entries: {}", GAMES_DB.len());

  let rom = include_bytes!("../roms/prince of persia.nes");

  let mut sha1 = sha1::Sha1::new();
  sha1.update(&rom[16..]);

  let hash = sha1.finalize()
    .to_vec()
    .iter()
    .map(|b| format!("{b:x}"))
    .collect::<String>();

  println!("Hashed value = {hash:?}");

  let res = GAMES_DB.get(&hash);

  println!("{res:?}");
}

#[test]
fn decode_test() {
  let file = include_bytes!("../utils/nes_games_db.bitcode.gzip").as_slice();

  let mut decode = flate2::read::GzDecoder::new(file);
  let mut buf = Vec::new();
  decode.read_to_end(&mut buf).unwrap();

  let parsed: Vec<GameData> = bitcode::decode(&buf).unwrap();
  println!("{:#?}", &parsed[0..10]);
}