// use core::fmt;
// use std::sync::LazyLock;
// use serde::{de::Visitor, Deserialize, Deserializer};

// #[derive(Deserialize, Debug, Default, Clone)]
// #[serde(default, rename_all = "camelCase")]
// pub struct Instruction {
//   pub opcode: u8,
//   #[serde(alias = "addressingMode")]
//   pub addressing: AddressingMode,
//   #[serde(alias = "mnemonics")]
//   #[serde(deserialize_with = "get_instr_first_name")]
//   pub name: &'static str,
//   #[serde(skip_deserializing)]
//   pub bytes: usize, 
//   pub cycles: usize,
//   pub page_boundary_cycle: bool,
//   pub illegal: bool,
// }

// #[derive(serde::Deserialize, Debug, Default, Clone, Copy, PartialEq)]
// #[serde(rename_all = "camelCase")]
// pub enum AddressingMode {
//   #[default]
//   #[serde(alias = "implied")]
//   Implied,
//   Accumulator,
//   Immediate,
//   #[serde(alias = "zeropage")]
//   ZeroPage,
//   #[serde(alias = "zeropageX")]
//   ZeroPageX,
//   #[serde(alias = "zeropageY")]
//   ZeroPageY,
//   Relative,
//   Absolute,
//   AbsoluteX,
//   AbsoluteY,
//   Indirect,
//   IndirectX,
//   IndirectY,
// }

// // https://www.reddit.com/r/learnrust/comments/15cq66f/can_you_partial_deserialize_a_vec/
// fn get_instr_first_name<D>(deserializer: D) -> Result<&'static str, D::Error> where D: Deserializer<'static> {
//   struct FirstElement;

//   impl Visitor<'static> for FirstElement {
//     type Value = &'static str;

//     fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//         formatter.write_str("an array of strings")
//     }

//     fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
//         where
//             A: serde::de::SeqAccess<'static>, {
//       let first = seq.next_element()?;
//       while let Some(_) = seq.next_element::<&'static str>()? {}

//       Ok(first.unwrap())
//     }
//   }

//   deserializer.deserialize_seq(FirstElement)
// }

// fn get_instr_len(inst: &Instruction) -> usize {
//   let mode = inst.addressing;
//   use AddressingMode::*;
//   match mode {
//     Implied | Accumulator => 1,

//     ZeroPage | ZeroPageX | ZeroPageY |
//     IndirectX | IndirectY |
//     Immediate | Relative => 2,

//     Absolute | AbsoluteX | AbsoluteY | 
//     Indirect => 3,
//   }
// }

// fn get_instructions() -> [Instruction; 256] {
//   let json = include_str!("../utils/instructions.json");
//   let mut deserialized = serde_json::from_str::<Vec<Instruction>>(json).unwrap();
  
//   deserialized.sort_by(|a, b| a.opcode.cmp(&b.opcode));
  
//   for instr in deserialized.iter_mut() {
//     instr.bytes = get_instr_len(instr);
//   }
  
//   deserialized.try_into().unwrap()
// }

// pub static INSTRUCTIONS: LazyLock<[Instruction; 256]> = LazyLock::new(get_instructions);
// pub const RMW_INSTRS: [&'static str; 18] = [
//   "ASL", "LSR", "ROL", "ROR", "INC", "DEC",
//   "SLO", "SRE", "RLA", "RRA", "ISB", "DCP",
//   "STA", "STX", "STY", "SHA", "SHX", "SHY"
// ];

// #[cfg(test)]
// mod tests {
//   use super::*;

//   #[test]
//   fn test_deserialize() -> Result<(), serde_json::Error>  {
//     let instrs = get_instructions();

//     println!("{:?}", instrs[2]);
//     Ok(())
//   }
// }

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum AddressingMode {
  #[default]
  Implied,
  Accumulator,
  Immediate,
  ZeroPage,
  ZeroPageX,
  ZeroPageY,
  Relative,
  Absolute,
  AbsoluteX,
  AbsoluteY,
  Indirect,
  IndirectX,
  IndirectY,
}

use AddressingMode::*;
pub const MODES_TABLE: [AddressingMode; 256] = [
  Implied,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Absolute,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Implied,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Implied,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Indirect,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageY,
  ZeroPageY,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteY,
  AbsoluteY,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageY,
  ZeroPageY,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteY,
  AbsoluteY,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
];