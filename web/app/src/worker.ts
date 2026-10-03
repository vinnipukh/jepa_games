// Generation and solving off the main thread. The accepted puzzle is decided by the solver's
// state-count limit inside wasm (D11); the page's timeout only guards the UI.
import init, * as wasm from '../../pkg/water_sort_web.js';
import type { WorkerRequest, WorkerResponse } from './protocol';

const ready = init();

function handle(req: WorkerRequest): string {
  if (req.kind === 'code') {
    const p = wasm.from_code(req.code);
    try {
      return p.to_json();
    } finally {
      p.free();
    }
  }
  const params = new wasm.Params(...req.params);
  try {
    const p = wasm.generate(req.generator, params, req.seed, req.strategy, req.layout, req.tier);
    try {
      return p.to_json();
    } finally {
      p.free();
    }
  } finally {
    params.free();
  }
}

self.onmessage = async (e: MessageEvent<WorkerRequest>) => {
  await ready;
  const req = e.data;
  const start = performance.now();
  let res: WorkerResponse;
  try {
    res = { id: req.id, ok: true, json: handle(req), millis: performance.now() - start };
  } catch (err) {
    res = { id: req.id, ok: false, error: err instanceof Error ? err.message : String(err) };
  }
  self.postMessage(res);
};
