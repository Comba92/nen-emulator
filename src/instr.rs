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
  pub name: String,
  #[serde(skip_deserializing)]
  pub bytes: usize, 
  pub cycles: usize,
  pub page_boundary_cycle: bool,
  pub illegal: bool,
}

#[derive(Deserialize, Debug, Default, Clone, Copy)]
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
  let json = include_str!("../utils/instructions.json");
  let mut deserialized = serde_json::from_str::<Vec<Instruction>>(json).unwrap();
  
  deserialized.sort_by(|a, b| a.opcode.cmp(&b.opcode));
  
  for instr in deserialized.iter_mut() {
    instr.bytes = get_instr_len(&instr);
  }
  
  deserialized.try_into().unwrap()
}


pub static INSTRUCTIONS: LazyLock<[Instruction; 256]> = LazyLock::new(get_instructions);

impl<M: Memory> Cpu<M> {
  pub fn execute(&mut self, code: u8, op: &mut Operand) {
    match code {
      0 => self.brk(op),
      1 | 5 | 9 | 13 | 17 | 21 | 25 | 29 => self.ora(op),
      2 | 18 | 34 | 50 | 66 | 82 | 98 | 114 | 146 | 178 | 210 | 242 => self.jam(op),
      3 | 7 | 15 | 19 | 23 | 27 | 31 => self.slo(op),
      4 | 12 | 20 | 26 | 28 | 52 | 58 | 60 | 68 | 84 | 90 | 92 
      | 100 | 116 | 122 | 124 | 128 | 130 | 137 | 194
      | 212 | 218 | 220 | 226 | 234 | 244 | 250 | 252 => self.nop(op),
      6 | 10 | 14 | 22 | 30 => self.asl(op),
      8 => self.php(op),
      11 | 43 => self.anc(op),
      16 => self.bpl(op),
      24 => self.clc(op),
      32 => self.jsr(op),
      33 | 37 | 41 | 45 | 49 | 53 | 57 | 61 => self.and(op),
      35 | 39 | 47 | 51 | 55 | 59 | 63 => self.rla(op),
      36 | 44 => self.bit(op),
      38 | 42 | 46 | 54 | 62 => self.rol(op),
      40 => self.plp(op),
      48 => self.bmi(op),
      56 => self.sec(op),
      64 => self.rti(op),
      65 | 69 | 73 | 77 | 81 | 85 | 89 | 93 => self.eor(op),
      67 | 71 | 79 | 83 | 87 | 91 | 95 => self.sre(op),
      70 | 74 | 78 | 86 | 94 => self.lsr(op),
      72 => self.pha(op),
      75 => self.alr(op),
      76 | 108 => self.jmp(op),
      80 => self.bvc(op),
      88 => self.cli(op),
      96 => self.rts(op),
      97 | 101 | 105 | 109 | 113 | 117 | 121 | 125 => self.adc(op),
      99 | 103 | 111 | 115 | 119 | 123 | 127 => self.rra(op),
      102 | 106 | 110 | 118 | 126 => self.ror(op),
      104 => self.pla(op),
      107 => self.arr(op),
      112 => self.bvs(op),
      120 => self.sei(op),
      129 | 133 | 141 | 145 | 149 | 153 | 157 => self.sta(op),
      131 | 135 | 143 | 151 => self.sax(op),
      132 | 140 | 148 => self.sty(op),
      134 | 142 | 150 => self.stx(op),
      136 => self.dey(op),
      138 => self.txa(op),
      139 => self.ane(op),
      144 => self.bcc(op),
      147 | 159 => self.sha(op),
      152 => self.tya(op),
      154 => self.txs(op),
      155 => self.tas(op),
      156 => self.shy(op),
      158 => self.shx(op),
      160 | 164 | 172 | 180 | 188 => self.ldy(op),
      161 | 165 | 169 | 173 | 177 | 181 | 185 | 189 => self.lda(op),
      162 | 166 | 174 | 182 | 190 => self.ldx(op),
      163 | 167 | 175 | 179 | 183 | 191 => self.lax(op),
      168 => self.tay(op),
      170 => self.tax(op),
      171 => self.lxa(op),
      176 => self.bcs(op),
      184 => self.clv(op),
      186 => self.tsx(op),
      187 => self.las(op),
      192 | 196 | 204 => self.cpy(op),
      193 | 197 | 201 | 205 | 209 | 213 | 217 | 221 => self.cmp(op),
      195 | 199 | 207 | 211 | 215 | 219 | 223 => self.dcp(op),
      198 | 206 | 214 | 222 => self.dec(op),
      200 => self.iny(op),
      202 => self.dex(op),
      203 => self.sbx(op),
      208 => self.bne(op),
      216 => self.cld(op),
      224 | 228 | 236 => self.cpx(op),
      225 | 229 | 233 | 237 | 241 | 245 | 249 | 253 => self.sbc(op),
      227 | 231 | 239 | 243 | 247 | 251 | 255 => self.isc(op),
      230 | 238 | 246 | 254 => self.inc(op),
      232 => self.inx(op),
      235 => self.usbc(op),
      240 => self.beq(op),
      248 => self.sed(op),
    }
  }
}


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