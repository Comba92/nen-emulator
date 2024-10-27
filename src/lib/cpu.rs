use super::instructions::{get_instructions, Instruction};

#[derive(Debug)]
pub struct Cpu {
  ip: u16,
  sp: u8,
  a: u8,
  x: u8,
  y: u8,
  status: u8,
  mem: [u8; 0xFFFF],

  //TODO: instructions should be static
  instructions: Vec<Instruction>,
}
impl Cpu {
  pub fn new() -> Self {
    Self {
      ip: 0xFFFC,
      sp: 0x00FD,
      a: 0, x: 0, y: 0, status: 0,
      mem: [0; 0xFFFF],
      instructions: get_instructions(),
    }
  }
}

const ROM_START: usize = 0x8000;

pub fn interpret(cpu: &mut Cpu, codes: Vec<u8>) {
  let mut iter = codes.iter();
  while let Some(opcode) = iter.next() {
    match opcode {
      0x00 => { break; },
      0x69 => {
        cpu.a += iter.next().unwrap();
      }
      _ => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cpu_test() {
    let mut cpu = Cpu::new();
    let codes = vec![0x69, 0x01, 0x69, 0x05];

    interpret(&mut cpu, codes);

    assert_eq!(cpu.a, 6);
  }
}