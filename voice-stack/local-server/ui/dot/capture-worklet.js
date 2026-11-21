// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
class GalionMic extends AudioWorkletProcessor {
  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (channel && channel.length) this.port.postMessage(channel.slice(0));
    return true;
  }
}
registerProcessor("galion-mic", GalionMic);
