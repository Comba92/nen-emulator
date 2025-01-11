class NesAudioWorker extends AudioWorkletProcessor {
  constructor() {
    super()
  
    this.samples = new Float32Array()

    this.port.onmessage = (e) => {
      this.samples.set(e.data)
    }
  }

  process(input, output, params) {
    console.log(samples)
    for (let i=0; i<input[0].length; i++) {
      output[0][i] = input[0][i]
    }

    return true
  }
}

registerProcessor('NesAudioWorker', NesAudioWorker)