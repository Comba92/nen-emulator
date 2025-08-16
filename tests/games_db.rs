use std::{fs, io::Write};

#[derive(serde::Serialize, serde::Deserialize)]
struct Root {
  nes20db: GameList, 
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GameList {
  game: Vec<GameEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GameEntry {
  prgram: Option<RamData>,
  prgnvram: Option<RamData>,
  chrram: Option<RamData>,
  chrnvram: Option<RamData>,

  rom: RomData,
  pcb: CartData,
  console: ConsoleData,
  expansion: ExpansionData,

  chrrom: Option<RomData>,
  prgrom: RomData,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RamData {
  _size: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CartData {
  _mapper: String,
  _submapper: String,
  _mirroring: String,
  _battery: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExpansionData {
  #[serde(rename = "_type")]
  kind: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ConsoleData {
  #[serde(rename = "_type")]
  kind: String,
  #[serde(rename = "_region")]
  region: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RomData {
  #[serde(rename = "_size")]
  size: String,
  #[serde(rename = "_crc32")]
  crc32: String,
  #[serde(rename = "_sha1")]
  sha1: String,
  #[serde(rename = "_sum16", default)]
  sum16: String,
}


#[derive(Default, Debug, serde::Serialize, bitcode::Encode, bincode::Encode)]
struct RomSection {
  size: usize,
  crc32: String,
  sha1: String,
  sum16: String
}
impl From<&RomData> for RomSection {
  fn from(value: &RomData) -> Self {
    Self {
      size: value.size.parse().unwrap(),
      crc32: value.crc32.clone(),
      sha1: value.sha1.clone(),
      sum16: value.sum16.clone(),
    }
  }
}

#[repr(u8)]
#[derive(Default, Debug, serde::Serialize, bitcode::Encode, bincode::Encode)]
enum Mirroring {
  #[default]
  Horizontal,
  Vertical,
  SingleScreen,
  FourScreens,
}
impl From<&str> for Mirroring {
  fn from(value: &str) -> Self {
    match value {
      "H" => Self::Horizontal,
      "V" => Self::Vertical,
      "1" => Self::SingleScreen,
      "4" => Self::FourScreens,
      _ => unreachable!()
    }
  }
}

#[derive(Default, Debug, serde::Serialize, bitcode::Encode, bincode::Encode)]
enum Region {
  #[default] NTSC,
  PAL,
  Multiple,
  Other,
}
impl From<usize> for Region {
  fn from(value: usize) -> Self {
    match value {
      0 => Self::NTSC,
      1 => Self::PAL,
      2 => Self::Multiple,
      _ => Self::Other,
    }
  }
}

#[derive(Default, Debug, serde::Serialize, bitcode::Encode, bincode::Encode)]
struct FinalEntry {
  title: String,
  category: String,

  rom: RomSection,
  prg: RomSection,
  chr: Option<RomSection>,

  prgram_size: Option<usize>,
  prgnvram_size: Option<usize>,
  chrram_size: Option<usize>,
  chrnvram_size: Option<usize>,

  mapper: usize,
  submapper: usize,
  mirroring: Mirroring,
  battery: bool,
  region: Region,
  
  console: u8,
  expansions: u8,
}

#[test]
fn parse_db() {
  let db_xml = include_str!("../utils/nes20db.xml");
  let names = db_xml.lines()
    .filter(|line| line.contains("<game><!--"))
    // .inspect(|line| println!("{line}"))
    .map(|line| {
      let (kind, name) = line
        .trim()
        .strip_prefix("<game><!-- ")
        .unwrap()
        .split_once('\\')
        .unwrap();

      (kind, name.strip_suffix(".nes -->").unwrap())
    })
    .collect::<Vec<_>>();

  let db_json = include_str!("../utils/nes20db.json");
  let entries: Root = serde_json::from_str(db_json).unwrap();

  let mut final_entries = Vec::new();

  for (entry, (category, name)) in entries.nes20db.game.iter().zip(names) {  
    let final_entry = FinalEntry {
      title: name.replace("&amp;", "&"),
      category: category.replace("&amp;", "&"),
      rom: RomSection::from(&entry.rom),

      prg: RomSection::from(&entry.prgrom),
      chr: entry.chrrom.as_ref().and_then(|x| Some(RomSection::from(x))),

      mapper: entry.pcb._mapper.parse().unwrap(),
      submapper: entry.pcb._submapper.parse().unwrap(),
      mirroring: Mirroring::from(entry.pcb._mirroring.as_str()),
      battery: entry.pcb._battery.parse::<u8>().unwrap() == 1,

      console: entry.console.kind.parse().unwrap(),
      region:  Region::from(entry.console.region.parse::<usize>().unwrap()),

      expansions: entry.expansion.kind.parse().unwrap(),

      chrram_size: entry.chrram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
      prgram_size: entry.prgram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
      chrnvram_size: entry.chrnvram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
      prgnvram_size: entry.prgnvram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
    };

    final_entries.push(final_entry);
  }

  fs::create_dir("./tests/db_tests").unwrap();

  let create_file = |name: &str| {
    let file = std::fs::File::create(format!("./db_tests/nes20db_good{}", name)).unwrap();
    std::io::BufWriter::new(file)
  };

  let out = create_file(".json");
  serde_json::to_writer_pretty(out, &final_entries).unwrap();

  let out = create_file(".cbor");
  ciborium::into_writer(&final_entries, out).unwrap();

  let mut out = create_file(".msgpack");
  rmp_serde::encode::write(&mut out, &final_entries).unwrap();

  let mut out = create_file(".bincode");
  let bin = bincode::encode_to_vec(&final_entries, bincode::config::standard()).unwrap();
  out.write_all(&bin).unwrap();
  let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
  gzip.write_all(&bin).unwrap();
  let gzip = gzip.finish().unwrap();
  let mut out = create_file(".bincode.gzip");
  out.write_all(&gzip).unwrap();

  let mut out = create_file(".bitcode");
  let bin = bitcode::encode(&final_entries);
  out.write_all(&bin).unwrap();
  let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
  gzip.write_all(&bin).unwrap();
  let gzip = gzip.finish().unwrap();
  let mut out = create_file(".bitcode.gzip");
  out.write_all(&gzip).unwrap();
}
