// const wasmFile = await fetch('./wasm-interface/target/wasm32-unknown-unknown/debug/wasm_interface.wasm')
// const wasm = await WebAssembly.instantiateStreaming(wasmFile, {})
// console.log("WASM LOADED: ", wasm)


// console.log(wasm.instance.exports)

import init, {JSEmu} from './pkg/nen_emulator.js'
console.log(await init())

let emu = JSEmu.new()
console.log(emu)
console.log(emu.test())