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
    let frame = new Uint8ClampedArray(instance.memory.buffer, screen, canvas.width*canvas.height*4)
    let image = new ImageData(frame, canvas.width, canvas.height) 
    ctx.putImageData(image, 0, 0)
    requestAnimationFrame(renderLoop)
}

if (!emu.paused) renderLoop()
