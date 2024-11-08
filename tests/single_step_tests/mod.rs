use core::fmt;
use std::fs;

use nen_emulator::{cpu::{Cpu, CpuFlags}, mem::Memory};
use prettydiff::diff_words;
use serde::Deserialize;


#[derive(Deserialize, Debug, PartialEq, Eq)]
struct CpuMock {
  pc: u16,
  #[serde(alias = "s")]
  sp: u8,
  a: u8,
  x: u8,
  y: u8,
  p: u8,
  ram: Vec<(u16, u8)>
}
impl fmt::Display for CpuMock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "{}", format!("{:?}", self))
    }
}
impl CpuMock {
  fn from_cpu(cpu: &Cpu) -> Self {
    Self {
      pc: cpu.pc, sp: cpu.sp, a: cpu.a, x: cpu.x, y: cpu.y, 
      p: cpu.p.bits(), ram: Vec::new(),
    }
  }
}

#[derive(Deserialize, Debug)]
struct Test {
  name: String,
  #[serde(alias = "initial")]
  start: CpuMock,
  #[serde(alias = "final")]
  end: CpuMock,
  cycles: Vec<(u16, u8, String)>
}

#[test]
fn cpu_test_one() {
  let json = include_str!("./tests/00.json");
  let test: Vec<Test> = serde_json::from_str(json).unwrap();

  let mut cpu = cpu_from_mock(&test[0].start);
  while cpu.cycles < test[0].cycles.len() {
    println!("{:?}", cpu);
    cpu.step();
  }
  let mut my_end = CpuMock::from_cpu(&cpu);
  for (addr, _) in &test[0].end.ram {
    my_end.ram.push((*addr, cpu.read(*addr)))
  }
  assert_eq!(test[0].end, my_end, 
    "Found error {:?}\n{}",
    test[0].name, diff_words(&my_end.to_string(), &test[0].end.to_string()));
}

fn cpu_from_mock(mock: &CpuMock) -> Cpu {
  let mut cpu = Cpu::with_ram64kb();
  cpu.a = mock.a;
  cpu.x = mock.x;
  cpu.y = mock.y;
  cpu.sp = mock.sp;
  cpu.pc = mock.pc;
  cpu.p = CpuFlags::from_bits_retain(mock.p);
  cpu.cycles = 0;
  for (addr, byte) in &mock.ram {
    cpu.write(*addr, *byte);
  }

  cpu
}

#[test]
fn cpu_test() {
  let mut dir = fs::read_dir("./tests/single_step_tests/tests")
    .expect("directory not found")
    .enumerate();

  while let Some((i, Ok(f))) = dir.next() {
    let json_test = fs::read(f.path()).expect("couldnt't read file");
    let tests: Vec<Test> = serde_json::from_slice(&json_test).expect("couldn't parse json");
    println!("Testing file {i}: {:?}", f.file_name());

    'testing: for test in tests.iter() {
      let mut cpu = cpu_from_mock(&test.start);
      while cpu.cycles < test.cycles.len() {
        cpu.step();
        if cpu.jammed { continue 'testing; }
      }

      let mut my_end = CpuMock::from_cpu(&cpu);
      for (addr, _) in &test.end.ram {
        my_end.ram.push((*addr, cpu.read(*addr)))
      }

      if my_end != test.end {
        let mut builder = colog::basic_builder();
        builder.filter_level(log::LevelFilter::Trace);
        builder.init();

        let mut log_cpu = cpu_from_mock(&test.start);
        while log_cpu.cycles < test.cycles.len() {
          log_cpu.step();
          if log_cpu.jammed { continue 'testing; }
        }
        panic!("Found error in file {:?}, test {:?}\n{}",
          f.file_name(), test.name, diff_words(&my_end.to_string(), &test.end.to_string())
        );
      }
    }
  }
}
