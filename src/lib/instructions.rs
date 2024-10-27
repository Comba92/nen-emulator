use std::sync::LazyLock;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Instruction {
  pub opcode: u8,
  #[serde(alias = "addressingMode")]
  pub addressing: AddressingMode,
  #[serde(alias = "mnemonics")]
  pub names: Vec<String>,
  pub cycles: usize,
  pub page_boundary_cycle: bool,
  pub illegal: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum AddressingMode {
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

pub static INSTRUCTIONS: LazyLock<[Instruction; 256]> = LazyLock::new(|| {
  let json = include_str!("instructions.json");
  let mut deserialized = serde_json::from_str::<Vec<Instruction>>(json).unwrap();

  deserialized.sort_by(|a, b| a.opcode.cmp(&b.opcode));
  deserialized.try_into().unwrap()
});

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