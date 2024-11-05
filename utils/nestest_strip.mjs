import * as fs from 'fs'

let log = fs.readFileSync('./utils/nestest.log')
let lines = log.toString().split('\n')
const left = 20
const right = 48
for (let i=0; i<lines.length; ++i) {
  let line = lines[i]
  lines[i] = line.substring(0, left) + ' '.repeat(right - left) + line.substring(right);
}

let out = lines.join('\n')

fs.appendFileSync('./utils/nestest_strip.log', out)
