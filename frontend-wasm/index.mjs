const inputRom = document.getElementById("rom-picker")
const pauseBtn = document.getElementById("play-pause")
const resetBtn = document.getElementById("reset")

const nesScreen = document.getElementById("nes-screen")
const nesCtx = nesScreen.getContext('2d')

const SCALING = 2
const SCREEN_WIDTH = 32*8
const SCREEN_HEIGHT = 30*8

nesScreen.width = SCREEN_WIDTH
nesScreen.height = SCREEN_HEIGHT

const keymap = [
    { key: 's', button: 1 },
    { key: 'd', button: 2 },
    { key: 'w', button: 4 },
    { key: 'e', button: 8 },
    { key: 'ArrowUp', button: 16 },
    { key: 'ArrowDown', button: 32 },
    { key: 'ArrowLeft', button: 64 },
    { key: 'ArrowRight', button: 128 },
    { key: ' ', button: 'pause' },
    { key: 'r', button: 'reset' },
]

window.addEventListener('keydown', event => {
    let pressed = keymap.filter(key => key.key === event.key)[0]
    if (pressed === undefined) { return }

    if (isNaN(pressed.button)) {
        if (pressed.button === 'pause') { emu.is_paused = !emu.is_paused }
        else if (pressed.button == 'reset') { emu.reset() }
    } else {
        emu.button_pressed(pressed.button)
    }
})

window.addEventListener('keyup', event => {
    let pressed = keymap.filter(key => key.key === event.key)[0]
    if (pressed === undefined) { return }

    if (isNaN(pressed.button)) {
        if (pressed.button === 'pause') { emu.is_paused = !emu.is_paused }
        else if (pressed.button == 'reset') { emu.reset() }
    } else {
        emu.button_released(pressed.button)
    }
})

// TODO: controller not working
window.addEventListener("gamepadconnected", (e) => {
    console.log(
        "Gamepad connected at index %d: %s. %d buttons, %d axes.",
        e.gamepad.index,
        e.gamepad.id,
        e.gamepad.buttons.length,
        e.gamepad.axes.length,
    );
});

import init, {Nes} from './frontend-wasm/pkg/nen_emulator.js'
const instance = await init()

let emu = Nes.boot_empty()
let screen = emu.get_raw_screen()
let animationId = null

inputRom.addEventListener('change', async event => {
    let rom = await inputRom.files[0].arrayBuffer()
    let bytes = new Uint8Array(rom)
    try {
        emu = Nes.boot_from_bytes(bytes)
        screen = emu.get_raw_screen()
        pauseBtn.innerText = '⏸️'
        animationId = renderLoop()
    } catch(err) {
        console.log(err)
        emu.is_paused = true
    }
})


pauseBtn.addEventListener('click', event => {
    if (emu.is_paused) {
        renderLoop()
    } else {
        cancelAnimationFrame(animationId)
    }
    emu.is_paused = !emu.is_paused
    pauseBtn.innerText = emu.is_paused ? '▶️' : '⏸️' 
})

resetBtn.addEventListener('click', event => {
    emu.reset()
    emu.is_paused = false
    pauseBtn.innerText = '⏸️'
})

const FRAME_MS = (1.0 / 60.0) * 1000

// https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Pixel_manipulation_with_canvas
function renderLoop() {
    let start = performance.now()
    
    emu.step_until_vblank()
    let frame = new Uint8ClampedArray(instance.memory.buffer, screen, nesScreen.width*nesScreen.height*4)
    let image = new ImageData(frame, nesScreen.width, nesScreen.height)
    nesCtx.putImageData(image, 0, 0)
    
    let elapsed_ms = performance.now() - start
    let delay = FRAME_MS - elapsed_ms

    // TODO: this shit doesnt work
    // setTimeout(
    //     () => { animationId = requestAnimationFrame(renderLoop) },
    //     delay > 0 ? delay : 0
    // )
    animationId = requestAnimationFrame(renderLoop)
}


function playRandomAudio() {
    var context = new (window.AudioContext || window.webkitAudioContext)();
    var osc = context.createOscillator(); // instantiate an oscillator
    osc.type = 'sine'; // this is the default - also square, sawtooth, triangle
    osc.frequency.value = 440; // Hz
    osc.connect(context.destination); // connect it to the destination
    osc.start(); // start the oscillator
    osc.stop(context.currentTime + 2); // stop 2 seconds after the current time
}

