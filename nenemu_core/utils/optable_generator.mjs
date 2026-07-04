import * as fs from 'fs';

let jsonFile = fs.readFileSync('instructions.json', 'utf8')
let json = JSON.parse(jsonFile)


let optable = []
for (let instr of json) {
  let mode = instr.addressingMode?.charAt(0).toUpperCase() + instr.addressingMode?.slice(1);

  if (!mode) {
    mode = "Implied"
  } else if (mode.startsWith("Zero")) {
    mode = mode.replace("page", "Page")
  }

  optable[instr.opcode] = mode
}

let res = ''
for (let entry of optable) {
  let s = `${entry},\n`
  res += s
}

console.log(res)