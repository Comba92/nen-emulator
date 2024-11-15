import * as fs from 'fs';

let jsonFile = fs.readFileSync('./utils/instructions.json', 'utf8')
let json = JSON.parse(jsonFile)

let names = json
  .map(inst => inst.mnemonics[0])
  .map(name => `map.insert("${name}", Cpu::${name.toLowerCase()});`)

let nodups = [... new Set(names)].join('\n')
console.log(nodups)

console.log('\n\n')
let groups = new Map()
for (let instr of json) {
  let name = instr.mnemonics[0]
  if (groups.has(name)) {
    let ops = groups.get(name)
    ops.push(instr.opcode)
  } else {
    groups.set(name, [instr.opcode])
  }
}

let arms = Array.from(groups).map(([name, opcodes]) => opcodes.join(' | ') + ' => self.' + name.toLowerCase() + '(op),')
console.log(arms.join('\n'))