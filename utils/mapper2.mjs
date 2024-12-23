import * as fs from 'fs';

let jsonFile = fs.readFileSync('./utils/instructions.json', 'utf8')
let json = JSON.parse(jsonFile)

let res = ''
for (let instr of json) {
  let addressing = (instr.addressingMode?.charAt(0).toUpperCase() + instr.addressingMode?.slice(1))
    ?? 'Implied'
  let name = instr.mnemonics[0].toLowerCase()
  let opcode = instr.opcode.toString(16).toUpperCase().padStart(2, '0')
  res += `0x${opcode} => self.${name}(op, ${addressing}::default()),\n`
}

console.log(res)