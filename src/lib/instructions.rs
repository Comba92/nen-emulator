use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Instruction {
  opcode: u8,
  #[serde(alias = "addressingMode")]
  addressing: Addressing,
  #[serde(alias = "mnemonics")]
  names: Vec<String>,
  cycles: usize,
  //TODO: check wtf is this
  page_boundary_cycle: bool,
  illegal: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub enum Addressing {
  #[default]
  #[serde(alias = "implied")]
  Implicit,
  Accumulator,
  Immediate,
  #[serde(alias = "zeropage")]
  ZeroPage,
  #[serde(alias = "zeropageX")]
  ZeroPageX,
  #[serde(alias = "zeropageY")]
  ZeroPageY,
  Relative,
  Absolute,
  AbsoluteX,
  AbsoluteY,
  Indirect,
  IndirectX,
  IndirectY,
}

pub fn get_instructions() -> Vec<Instruction> {
  let json = include_str!("instructions.json");
  serde_json::from_str::<Vec<Instruction>>(json).unwrap()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_deserialize() -> Result<(), serde_json::Error>  {
    let json = include_str!("instructions.json");
    let instrs = serde_json::from_str::<Vec<Instruction>>(json)?;

    println!("{:?}", instrs[0]);
    Ok(())
  }
}