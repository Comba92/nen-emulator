const inputRom = document.getElementById("rom-picker")
const pauseBtn = document.getElementById("play-pause")
const resetBtn = document.getElementById("reset")

const nesScreen = document.getElementById("nes-screen")
const nesVideoCtx = nesScreen.getContext('2d')
let nesAudioCtx  = null
let audioNode = null

const SCREEN_WIDTH = 32*8
const SCREEN_HEIGHT = 30*8

nesScreen.width = SCREEN_WIDTH
nesScreen.height = SCREEN_HEIGHT

const keymap = [
    { key: 's', button: 1 },
    { key: 'd', button: 2 },
    { key: 'e', button: 4 },
    { key: 'w', button: 8 },
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
    console.warn(
        "Gamepad connected at index %d: %s. %d buttons, %d axes.",
        e.gamepad.index,
        e.gamepad.id,
        e.gamepad.buttons.length,
        e.gamepad.axes.length,
    );
});

import init, {Nes} from './pkg/nen_emulator.js'
const instance = await init()

let emu = Nes.boot_empty()
let nesScreenPtr = emu.get_raw_screen()
let animationId = null

inputRom.addEventListener('change', async event => {
    let rom = await inputRom.files[0].arrayBuffer()
    let bytes = new Uint8Array(rom)
    try {
        emu = Nes.boot_from_bytes(bytes)
        nesScreenPtr = emu.get_raw_screen()
        pauseBtn.innerText = '⏸️'

        nesAudioCtx = new AudioContext()

        // try {
        //     nesAudioCtx = new AudioContext()
        //     await nesAudioCtx
        //     .audioWorklet
        //     .addModule('audioWorker.js')
        //     audioNode = new AudioWorkletNode(nesAudioCtx, 'NesAudioWorker')
        //     audioNode.connect(nesAudioCtx.destination)

        //     nesAudioCtx.resume()
        // } catch (e) {
        //     console.error("Couldn't start audio worker")
        // }

        animationId = renderLoop()
    } catch(err) {
        console.error(err)
        emu.is_paused = true
    }
})


pauseBtn.addEventListener('click', event => {
    if (emu.is_paused) {
        animationId = renderLoop()
    } else {
        cancelAnimationFrame(animationId)
    }
    emu.is_paused = !emu.is_paused
    pauseBtn.innerText = emu.is_paused ? '▶️' : '⏸️' 
})

resetBtn.addEventListener('click', event => {
    if (emu.is_paused) {
        animationId = renderLoop()
    }
    emu.is_paused = false
    emu.reset()
    pauseBtn.innerText = '⏸️'
})

const FRAME_MS = 1000/60
let then = 0

// Thanks to: https://github.com/jeffrey-xiao/neso-web/blob/master/src/index.js
function renderLoop() {
    animationId = requestAnimationFrame(renderLoop)

    let now = Date.now()
    let elapsed = now - then

    if (elapsed > FRAME_MS) {
        then = now - (elapsed % FRAME_MS)

        emu.step_until_vblank()
        renderVideo()
        // renderAudio()   
    }
}

function renderVideo() {
    let frame = new Uint8ClampedArray(
        instance.memory.buffer,
        nesScreenPtr,
        nesScreen.width*nesScreen.height*4
    )
    let image = new ImageData(frame, nesScreen.width, nesScreen.height)
    nesVideoCtx.putImageData(image, 0, 0)
}

function renderAudio() {
    let samplesCount = emu.get_samples_count()
    let frame = new Float32Array(
        instance.memory.buffer,
        emu.get_raw_samples(),
        samplesCount
    )
    let buffer = nesAudioCtx.createBuffer(1, samplesCount, 44100)
    buffer.copyToChannel(frame, 0, 0)
    emu.consume_samples()

    let audioNode = nesAudioCtx.createBufferSource()
    audioNode.connect(nesAudioCtx.destination)
    audioNode.buffer = buffer
    audioNode.start()
}
