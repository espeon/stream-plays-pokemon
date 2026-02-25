// GBA audio worklet processor.
// Runs on the AudioWorklet thread; receives PCM chunks from the main thread
// via postMessage and outputs them to the DAC with nearest-neighbor resampling.
//
// The GBA produces s16le stereo at 32768 Hz. The AudioContext runs at the
// device rate (usually 48000 Hz). We resample here.

const PROCESSOR_CODE = /* js */ `
class GbaAudioProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    // Ring buffer: Float32 stereo interleaved, pre-allocated (~1s at 32768 Hz)
    this._buf = new Float32Array(32768 * 2);
    this._writePos = 0;
    this._readPos = 0;
    this._count = 0;
    // Resampling state
    this._resamplePos = 0;
    this._lastL = 0;
    this._lastR = 0;
    this._ratio = 0;
    // Don't start draining until we have this many GBA stereo samples buffered.
    // ~100ms at 32768 Hz keeps us ahead of WS jitter without adding much latency.
    this._preFillTarget = 32768 * 0.05 * 2;
    this._preFilled = false;

    this.port.onmessage = (e) => {
      const samples = e.data; // Int16Array, interleaved stereo at 32768 Hz
      const cap = this._buf.length;
      for (let i = 0; i < samples.length; i += 2) {
        if (this._count + 2 > cap) break; // drop oldest if full
        this._buf[this._writePos] = samples[i] / 32768;
        this._buf[(this._writePos + 1) % cap] = samples[i + 1] / 32768;
        this._writePos = (this._writePos + 2) % cap;
        this._count += 2;
      }
      if (!this._preFilled && this._count >= this._preFillTarget) {
        this._preFilled = true;
      }
    };
  }

  process(_inputs, outputs) {
    if (this._ratio === 0) {
      this._ratio = 32768 / sampleRate;
    }
    const out = outputs[0];
    const outL = out[0];
    const outR = out[1] ?? out[0];
    const cap = this._buf.length;

    for (let i = 0; i < outL.length; i++) {
      if (this._preFilled) {
        this._resamplePos += this._ratio;
        while (this._resamplePos >= 1.0 && this._count >= 2) {
          this._lastL = this._buf[this._readPos];
          this._lastR = this._buf[(this._readPos + 1) % cap];
          this._readPos = (this._readPos + 2) % cap;
          this._count -= 2;
          this._resamplePos -= 1.0;
        }
        // If buffer runs dry, hold the last sample (no click) and re-arm prefill
        if (this._count < 2) {
          this._preFilled = false;
        }
      }
      // Output last known sample — silence before prefill, held sample on underrun
      outL[i] = this._lastL;
      outR[i] = this._lastR;
    }
    return true;
  }
}

registerProcessor('gba-audio-processor', GbaAudioProcessor);
`;

export function createAudioWorkletBlobUrl(): string {
  const blob = new Blob([PROCESSOR_CODE], { type: "application/javascript" });
  return URL.createObjectURL(blob);
}
