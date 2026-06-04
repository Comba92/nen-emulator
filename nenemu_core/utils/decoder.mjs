import fs from 'fs'

let file = fs.readFileSync('utils/6502_instructions.json')
let json = JSON.parse(file)

let res1 = json.opcodes
  .filter(x => !x.illegal)
  .map(x => `${x.opcode_hex} => self.${x.name.toLowerCase()}(),`)
  .join('\n')

let res2 = json.opcodes
  .filter(x => x.illegal)
  .map(x => `${x.opcode_hex} => self.${x.name.toLowerCase()}(),`)
  .join('\n')

console.log("Decoded:")
console.log(res1)
console.log()
console.log(res2)

let legals = json.opcodes
  .filter(x => !x.illegal)
  .map(x => `"${x.opcode_hex.slice(2, 4)}"`)
  .join(", ")

console.log("Legals array:")
console.log("[" + legals + "]")

let modes = json.opcodes
  .map(x => x.addressing_mode)
  .join(", ")

let cycles = json.opcodes
  .map(x => `${x.cycles}`)
  .join(", ")

console.log("Cycles table:")
console.log(cycles)