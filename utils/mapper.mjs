import * as fs from 'fs';

let jsonFile = fs.readFileSync('./utils/instructions.json', 'utf8')
let json = JSON.parse(jsonFile)

let names = json
  .map(inst => inst.mnemonics[0])
  .map(name => `map.insert("${name}", Cpu::${name.toLowerCase()});`)

let nodups = [... new Set(names)].join('\n')
console.log(nodups)