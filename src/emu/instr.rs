#![allow(dead_code)]

use std::{collections::HashMap, sync::LazyLock};
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
use super::cpu::*;

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
pub static OPCODES_MAP: LazyLock<HashMap<&'static str, InstructionFn>> = LazyLock::new(|| {
  let mut map: HashMap<&'static str, InstructionFn> = HashMap::new();
  
  map.insert("BRK", brk);
  map.insert("ORA", ora);
  map.insert("NOP", nop);
  map.insert("ASL", asl);
  map.insert("PHP", php);
  map.insert("BPL", bpl);
  map.insert("CLC", clc);
  map.insert("JSR", jsr);
  map.insert("AND", and);
  map.insert("BIT", bit);
  map.insert("ROL", rol);
  map.insert("PLP", plp);
  map.insert("SEC", sec);
  map.insert("RTI", rti);
  map.insert("EOR", eor);
  map.insert("LSR", lsr);
  map.insert("PHA", pha);
  map.insert("JMP", jmp);
  map.insert("BVC", bvc);
  map.insert("CLI", cli);
  map.insert("RTS", rts);
  map.insert("ADC", adc);
  map.insert("ROR", ror);
  map.insert("PLA", pla);
  map.insert("BVS", bvs);
  map.insert("SEI", sei);
  map.insert("STA", sta);
  map.insert("STY", sty);
  map.insert("STX", stx);
  map.insert("DEY", dey);
  map.insert("TXA", txa);
  map.insert("BCC", bcc);
  map.insert("TYA", tya);
  map.insert("TXS", txs);
  map.insert("LDY", ldy);
  map.insert("LDA", lda);
  map.insert("LDX", ldx);
  map.insert("TAY", tay);
  map.insert("TAX", tax);
  map.insert("BCS", bcs);
  map.insert("CLV", clv);
  map.insert("TSX", tsx);
  map.insert("CPY", cpy);
  map.insert("CMP", cmp);
  map.insert("DEC", dec);
  map.insert("INY", iny);
  map.insert("DEX", dex);
  map.insert("BNE", bne);
  map.insert("CLD", cld);
  map.insert("CPX", cpx);
  map.insert("SBC", sbc);
  map.insert("INC", inc);
  map.insert("INX", inx);
  map.insert("BEQ", beq);
  map.insert("SED", sed);

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