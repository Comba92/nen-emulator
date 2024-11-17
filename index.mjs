const inputRom = document.getElementById("rom-picker")
const canvas = document.getElementById("nes-screen")
const ctx = canvas.getContext('2d')
canvas.width = 32*8
canvas.height = 30*8

import init, {Emu} from './pkg/nen_emulator.js'
const instance = await init()

let emu = Emu.empty()
let screen = emu.get_raw_screen()

inputRom.addEventListener('change', async event => {
    let rom = await inputRom.files[0].arrayBuffer()
    let bytes = new Uint8Array(rom)
    emu = Emu.from_bytes(bytes)
    screen = emu.get_raw_screen()
})

function renderLoop() {
    if (!emu.paused) emu.step_until_vblank()
    let frame = new Uint8Array(instance.memory.buffer, screen, 32*8*30*8*3)

    for (let row=0; row<32*8; row++) {
        for (let col=0; col<32*8; col++) {
            let idx = (row*32*8 + col)*3;
            let r = frame[idx]
            let g = frame[idx+1]
            let b = frame[idx+2]
            ctx.fillStyle = 'rgb(' + [r,g,b].join(' ') + ')' 
            ctx.fillRect(col, row, 1, 1)
        }
    }
    requestAnimationFrame(renderLoop)
}

if (!emu.paused) renderLoop()
