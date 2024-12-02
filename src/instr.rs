use core::fmt;
use std::sync::LazyLock;
use serde::{de::Visitor, Deserialize, Deserializer};

use crate::{cpu::{Cpu, Operand}, mem::Memory};

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Instruction {
  pub opcode: u8,
  #[serde(alias = "addressingMode")]
  pub addressing: AddressingMode,
  #[serde(alias = "mnemonics")]
  #[serde(deserialize_with = "get_instr_first_name")]
  pub name: &'static str,
  #[serde(skip_deserializing)]
  pub bytes: usize, 
  pub cycles: usize,
  pub page_boundary_cycle: bool,
  pub illegal: bool,
}

#[derive(Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
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

// https://www.reddit.com/r/learnrust/comments/15cq66f/can_you_partial_deserialize_a_vec/
fn get_instr_first_name<D>(deserializer: D) -> Result<&'static str, D::Error> where D: Deserializer<'static> {
  struct FirstElement;

  impl Visitor<'static> for FirstElement {
    type Value = &'static str;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an array of strings")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'static>, {
      let first = seq.next_element()?;
      while let Some(_) = seq.next_element::<&'static str>()? {}

      Ok(first.unwrap())
    }
  }

  deserializer.deserialize_seq(FirstElement)
}

fn get_instr_len(inst: &Instruction) -> usize {
  let mode = inst.addressing;
  use AddressingMode::*;
  match mode {
    Implicit | Accumulator => 1,

    ZeroPage | ZeroPageX | ZeroPageY |
    IndirectX | IndirectY |
    Immediate | Relative => 2,

    Absolute | AbsoluteX | AbsoluteY | 
    Indirect => 3,
  }
}

fn get_instructions() -> [Instruction; 256] {
  let json = include_str!("../utils/instructions.json");
  let mut deserialized = serde_json::from_str::<Vec<Instruction>>(json).unwrap();
  
  deserialized.sort_by(|a, b| a.opcode.cmp(&b.opcode));
  
  for instr in deserialized.iter_mut() {
    instr.bytes = get_instr_len(instr);
  }
  
  deserialized.try_into().unwrap()
}


pub static INSTRUCTIONS: LazyLock<[Instruction; 256]> = LazyLock::new(get_instructions);


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_deserialize() -> Result<(), serde_json::Error>  {
    let instrs = get_instructions();

    println!("{:?}", instrs[2]);
    Ok(())
  }
}