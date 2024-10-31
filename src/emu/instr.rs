#![allow(dead_code)]

use std::{collections::HashMap, sync::LazyLock};
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};

use super::cpu::{Cpu, InstrFn};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Instruction {
  pub opcode: u8,
  #[serde(alias = "addressingMode")]
  pub addressing: AddressingMode,
  #[serde(alias = "mnemonics")]
  #[serde(deserialize_with = "get_instr_first_name")]
  pub name: String,
  #[serde(skip_deserializing)]
  pub bytes: usize, 
  pub cycles: usize,
  // pub page_boundary_cycle: bool,
  // pub illegal: bool,
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

// https://www.reddit.com/r/learnrust/comments/15cq66f/can_you_partial_deserialize_a_vec/
fn get_instr_first_name<'de, D>(deserializer: D) -> Result<String, D::Error> where D: Deserializer<'de> {
  struct FirstElement;

  impl<'de> Visitor<'de> for FirstElement {
    type Value = String;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an array of strings")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>, {
      let first = seq.next_element()?;
      while let Some(_) = seq.next_element::<String>()? {}

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
  let json = include_str!("../../utils/instructions.json");
  let mut deserialized = serde_json::from_str::<Vec<Instruction>>(json).unwrap();
  
  deserialized.sort_by(|a, b| a.opcode.cmp(&b.opcode));
  
  for instr in deserialized.iter_mut() {
    instr.bytes = get_instr_len(&instr);
  }
  
  deserialized.try_into().unwrap()
}


pub static INSTRUCTIONS: LazyLock<[Instruction; 256]> = LazyLock::new(get_instructions);
pub static INSTR_TO_FN: LazyLock<HashMap<&'static str, InstrFn>> = LazyLock::new(|| {
  let mut map: HashMap<&'static str, InstrFn> = HashMap::new();
  
  map.insert("BRK", Cpu::brk);
  map.insert("ORA", Cpu::ora);
  map.insert("NOP", Cpu::nop);
  map.insert("ASL", Cpu::asl);
  map.insert("PHP", Cpu::php);
  map.insert("BPL", Cpu::bpl);
  map.insert("CLC", Cpu::clc);
  map.insert("JSR", Cpu::jsr);
  map.insert("AND", Cpu::and);
  map.insert("BIT", Cpu::bit);
  map.insert("ROL", Cpu::rol);
  map.insert("PLP", Cpu::plp);
  map.insert("BMI", Cpu::bmi);
  map.insert("SEC", Cpu::sec);
  map.insert("RTI", Cpu::rti);
  map.insert("EOR", Cpu::eor);
  map.insert("LSR", Cpu::lsr);
  map.insert("PHA", Cpu::pha);
  map.insert("JMP", Cpu::jmp);
  map.insert("BVC", Cpu::bvc);
  map.insert("CLI", Cpu::cli);
  map.insert("RTS", Cpu::rts);
  map.insert("ADC", Cpu::adc);
  map.insert("ROR", Cpu::ror);
  map.insert("PLA", Cpu::pla);
  map.insert("BVS", Cpu::bvs);
  map.insert("SEI", Cpu::sei);
  map.insert("STA", Cpu::sta);
  map.insert("STY", Cpu::sty);
  map.insert("STX", Cpu::stx);
  map.insert("DEY", Cpu::dey);
  map.insert("TXA", Cpu::txa);
  map.insert("BCC", Cpu::bcc);
  map.insert("TYA", Cpu::tya);
  map.insert("TXS", Cpu::txs);
  map.insert("LDY", Cpu::ldy);
  map.insert("LDA", Cpu::lda);
  map.insert("LDX", Cpu::ldx);
  map.insert("TAY", Cpu::tay);
  map.insert("TAX", Cpu::tax);
  map.insert("BCS", Cpu::bcs);
  map.insert("CLV", Cpu::clv);
  map.insert("TSX", Cpu::tsx);
  map.insert("CPY", Cpu::cpy);
  map.insert("CMP", Cpu::cmp);
  map.insert("DEC", Cpu::dec);
  map.insert("INY", Cpu::iny);
  map.insert("DEX", Cpu::dex);
  map.insert("BNE", Cpu::bne);
  map.insert("CLD", Cpu::cld);
  map.insert("CPX", Cpu::cpx);
  map.insert("SBC", Cpu::sbc);
  map.insert("INC", Cpu::inc);
  map.insert("INX", Cpu::inx);
  map.insert("BEQ", Cpu::beq);
  map.insert("SED", Cpu::sed);

  map
});


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