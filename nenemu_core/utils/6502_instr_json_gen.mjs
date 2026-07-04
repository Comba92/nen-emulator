import * as fs from 'fs';

let jsonFile = fs.readFileSync('instructions.json', 'utf8')
let json = JSON.parse(jsonFile)

/*
enum AddressingMode {}
enum StatusFlag {}

AddressingModeDescr {
  kind: AddressingMode,
  instr_size: uint,
  cycles: uint,
  description: string,
  dummy_reads: DummyRead,
}

enum DummyRead {
  Always,
  AtPageCross,
  Never,
}

Instruction {
  opcode: uint
  name: string,
  name_long: string,
  alt_names: [string],
  addressing_mode: AddressingMode,
  byte_size: uint
  cycles: uint,
  cycles_excluded_mode: uint,
  illegal: bool,
  dummy_reads: DummyRead,
  dummy_writes: bool,
  flags: [StatusFlag]
  description: string,
  online_docs: [string],
}
*/

json = json
.map(i => {
  // let mode = i.addressingMode?.charAt(0).toUpperCase() + i.addressingMode?.slice(1);
  let mode = i.addressingMode?.charAt(0).toUpperCase() + i.addressingMode?.slice(1);
  if (!mode) {
    mode = "Implied"
  } else if (mode.startsWith("Zero")) {
    mode = mode.replace("page", "Page")
  }

  let mode_dummy_reads  = ''
  let byte_size = 1
  let mode_cycles = 2
  let mode_url = []
  switch (mode) {
    case 'Implied':
      mode_dummy_reads = 'Always'
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_implied')
      break;

    case 'Accumulator':
      mode_dummy_reads = 'Always'
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_implied')

      break;
    
    case 'Immediate':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_immediate')
      mode_dummy_reads = 'Never'
      break

    case 'Relative':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_relative')  
      mode_dummy_reads = 'Never'
      break

    case 'ZeroPage':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_zeropage')
      mode_dummy_reads = 'Never'
      byte_size = 2
      break

    case 'ZeroPageX':
    case 'ZeroPageY':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_zeropage_indexed')
      mode_dummy_reads = 'AtPageCross'
      byte_size = 3
      break

    case 'Absolute':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_absolute')
      mode_dummy_reads = 'Never'
      byte_size = 3
      mode_cycles = 3
      break
      
    case 'AbsoluteX':
    case 'AbsoluteY':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_indexed')
      mode_dummy_reads = 'AtPageCross'
      byte_size = 3
      mode_cycles = 3
      break

    case 'Indirect':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_indirect')
      mode_dummy_reads = 'Never'
      byte_size = 3
      mode_cycles = 5
      break      

    case 'IndirectX':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_preindexed_indirect')
      mode_dummy_reads = 'Always'
      byte_size = 2
      mode_cycles = 5
      break

    case 'IndirectY':
      mode_url.push('https://www.masswerk.at/6502/6502_instruction_set.html#modes_postindexed_indirect')
      mode_dummy_reads = 'AtPageCross'
      byte_size = 2
      mode_cycles = 4
      break
  }

  let name_long = ''
  let inst_cycles = 0
  let branch = false
  let flags = ''
  let dummy_reads = false
  let dummy_writes = false
  let mode_desc = ''

  // timings:  https://www.nesdev.org/6502_cpu.txt
  switch (i.mnemonics[0]) {
    case "ADC":
      name_long = "Add with carry";
      flags = "CZVN"
      break;
    case "AND":
      name_long = "Logical AND";
      flags = "ZN"
      break;
    case "ASL":
      name_long = "Arithmetic shift left"; 
      dummy_writes = true
      flags = "CZN"
      break;
    case "BCC":
      name_long = "Branch if carry clear"; 
      break;
    case "BCS":
      name_long = "Branch if carry set";
      break;
    case "BEQ":
      name_long = "Branch if equal";
      break;
    case "BIT":
      name_long = "Bit test";
      flags = "ZVN"
      break;
    case "BMI":
      name_long = "Branch if minus";
      break;
    case "BNE":
      name_long = "Branch if not equal"; 
      break;
    case "BPL":
      name_long = "Branch if positive";
      break;
    case "BRK":
      name_long = "Break / Software Interrupt";
      inst_cycles = 5
      flags = "B"
      break;
    case "BVC":
      name_long = "Branch if overflow clear"; 
      break;
    case "BVS":
      name_long = "Branch if overflow set"; 
      break;
    case "CLC":
      name_long = "Clear carry";
      flags = "C"
      break;
    case "CLD":
      name_long = "Clear decimal mode";
      flags = "D"
      break;
    case "CLI":
      name_long = "Clear interrupt disable";
      flags = "I"
      break;
    case "CLV":
      name_long = "Clear overflow flag"; 
      flags = "V"
      break;
    case "CMP":
      name_long = "Compare";
      flags = "CZN"
      break;
    case "CPX":
      name_long = "Compare X register";
      flags = "CZN"
      break;
    case "CPY":
      name_long = "Compare Y register";
      flags = "CZN"
      break;
    case "DEC":
      name_long = "Decrement memory";
      flags = "ZN"
      break;
    case "DEX":
      name_long = "Decrement X register";
      flags = "ZN"
      break;
    case "DEY":
      name_long = "Decrement Y register";
      flags = "ZN"
      break;
    case "EOR":
      name_long = "Logical Exclusive OR (XOR)";
      flags = "ZN"
      break;
    case "INC":
      name_long = "Increment memory";
      flags = "ZN"
      break;
    case "INX":
      name_long = "Increment X register";
      flags = "ZN"
      break;
    case "INY":
      name_long = "Increment Y register";
      flags = "ZN"
      break;
    case "JMP":
      name_long = "Unconditional Jump"; 
      break;
    case "JSR":
      name_long = "Jump to subroutine";
      inst_cycles = 3
      break;
    case "LDA":
      name_long = "Load accumulator";
      flags = "ZN"
      break;
    case "LDX":
      name_long = "Load X register";
      flags = "ZN" 
      break;
    case "LDY":
      name_long = "Load Y register";
      flags = "ZN"
      break;
    case "LSR":
      name_long = "Logical shift right";
      dummy_writes = true
      flags = "CZN"
      break;
    case "NOP":
      name_long = "No operation"; 
      break;
    case "ORA":
      name_long = "Logical Inclusive OR";
      flags = "ZN" 
      break;
    case "PHA":
      name_long = "Push accumulator";
      inst_cycles = 1
      break;
    case "PLA":
      name_long = "Pull accumulator";
      inst_cycles = 2
      flags = "ZN"
      break;
    case "PHP": 
      name_long = "Push processor status";
      inst_cycles = 1
      break;
    case "PLP": 
      name_long = "Pull processor status";
      inst_cycles = 2
      flags = "CZIDBVN"
      break;
    case "ROL":
      name_long = "Rotate left";
      dummy_writes = true
      flags = "ZN"
      break;
    case "ROR":
      name_long = "Rotate right";
      dummy_writes = true
      flags = "ZN"
      break;
    case "RTI":
      name_long = "Return from interrupt";
      inst_cycles = 4
      flags = "CZIDBVN"
      break;
    case "RTS":
      name_long = "Return from subroutine"; 
      inst_cycles = 4
      break;
    case "SBC":
      name_long = "Subtract with carry";
      flags = "CZVN"
      break;
    case "SEC":
      name_long = "Set carry"; 
      flags = "C"
      break;
    case "SED":
      name_long = "Set decimal flag";
      flags = "D"
      break;
    case "SEI":
      name_long = "Set interrupt disable";
      flags = "I"
      break;
    case "STA":
      name_long = "Store accumulator";
      break;
    case "STX":
      name_long = "Store X register";
      break;
    case "STY":
      name_long = "Store Y register";
      break;
    case "TAX":
      name_long = "Transfer acc to X"; 
      break;
    case "TAY":
      name_long = "Transfer acc to Y"; 
      break;
    case "TSX":
      name_long = "Transfer stack pointer to X"; 
      break;
    case "TXA":
      name_long = "Transfer X to acc"; 
      break;
    case "TYA":
      name_long = "Transfer Y to acc"; 
      break;
  }

  branch = name_long.startsWith("Branch")

  let name = i.mnemonics[0]
  let urls = []

  if (!i.illegal) {
    urls = urls.concat([
      'https://www.nesdev.org/wiki/Instruction_reference#' + name,
      'http://www.6502.org/users/obelisk/6502/reference.html#' + name,
    ])
  }

  urls = urls.concat([
    'https://www.masswerk.at/6502/6502_instruction_set.html#' + name,
    // 'https://www.pagetable.com/c64ref/6502/?tab=2#' + name,
  ])

  let cycles = mode_cycles + inst_cycles
  if (!["Implied", "Immediate", "Accumulator", "Relative"].includes(mode)) {
    if (["LDA","LDX","LDY","EOR","AND","ORA","ADC","SBC","CMP","BIT","LAX","NOP"].includes(name)) {
      cycles += 1
    }

    if (["ASL", "LSR", "ROL", "ROR", "INC", "DEC", "SLO", "SRE", "RLA", "RRA", "ISB", "DCP"].includes(name)) {
      dummy_writes = true;
      if (mode === 'AbsoluteX' || mode === 'AbsoluteY') dummy_reads = true;
      cycles += 3
    }

    if (["STA", "SAX"].includes(name)) {
      if (mode === 'IndirectY') dummy_reads = true
      cycles += 1
    }
  }

  let all_names = i.mnemonics
  if (!i.illegal) all_names = []

  return {
    opcode: i.opcode,
    name,
    name_long,
    all_names,
    addressing_mode: mode,
    mode_cycles,
    mode_dummy_reads,
    mode_desc,
    mode_url,
    byte_size,
    cycles: i.cycles ?? mode_cycles,
    illegal: i.illegal,
    branch,
    dummy_reads,
    dummy_writes,
    flags: flags.split(''),
    online_docs: urls,
  }
})

let modes = new Map(json.map(i => [i.addressing_mode, {
  kind: i.addressing_mode,
  size_bytes: i.byte_size,
  cycles: i.mode_cycles,
  dummy_reads: i.mode_dummy_reads,
  online_documentation: i.mode_url,
}]))

let instrs = new Map(json.map(i => [i.name, {
  name: i.name,
  name_long: i.name_long,
  all_names: i.all_names,
  flags: i.flags,
  illegal: i.illegal,
  // branch: i.branch,
  online_documentation: i.online_docs,
  opcodes: []
}]))

let opcodes = new Map(json.map(i => [i.opcode, {
  opcode: i.opcode,
  opcode_hex: '0x' + i.opcode.toString(16).padStart(2, '0'),
  name: i.name,
  addressing_mode: i.addressing_mode,
  cycles: i.cycles,
  dummy_reads: i.dummy_reads,
  dummy_writes: i.dummy_writes,
  illegal: i.illegal,
}]))

let merged = json.map(i => {
  return {
    name: i.name,
    name_long: i.name_long,
    
    opcode: i.opcode,
    opcode_hex: '0x' + i.opcode.toString(16).padStart(2, '0'),
    
    addressing_mode: i.addressing_mode,
    cycles: i.cycles,
    dummy_reads: i.dummy_reads,
    dummy_writes: i.dummy_writes,
    
    all_names: i.all_names,
    status_flags: i.flags,
    illegal: i.illegal,
    // branch: i.branch,
    online_documentation: i.online_docs,
  }
})


for (let op of opcodes.values()) {
  let instr = instrs.get(op.name)
  // delete op.name
  // instr.opcodes.push(op)
  instr.opcodes.push(op.opcode)
}

modes =  Array.from(modes.values())
modes.sort((a, b) => a.kind.localeCompare(b.kind))

instrs = Array.from(instrs.values())
instrs.sort((a, b) => a.name.localeCompare(b.name))

opcodes = Array.from(opcodes.values())
opcodes.sort((a, b) => a.opcode - b.opcode)

merged.sort((a,b) => a.opcode - b.opcode)

json = {
  modes, opcodes: merged,
}

// let cpu_memmap = [
//   {
//     device: "RAM",
//     size: 2 * 1024,
//     size_kb: "2KB",
//     size_hex: (2 * 1024).toString(16),
//     start: 0,
//     end: 2 * 1024,
//     range: "$0000 - $07ff",
//     mirrors: {
//       count: 3,
//       start: 2 * 1024,
//       end: 8 * 1024,
//       range: "$0800 - $1fff",
//     }
//   },

//   {
//     device: "PPU registers",
//     size: 8,
//     size_kb: "8B",
//     size_hex: "8",
//     start: 8 * 1024,
//     end: 8 * 1024 + 8,
//     range: "$2000 - $2007",
//     mirrors: {
//       count: 1024,
//       start: 8 * 1024 + 8,
//       end: 
//     }
//   }
// ]

fs.writeFileSync('6502_instructions.json', JSON.stringify(json, null, 2))