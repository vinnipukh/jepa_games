// Page-side client of the generation worker.
import type { CodeRequest, GenerateRequest, WorkerResponse } from './protocol';

/** UI guard only: a request still running after this long is abandoned (the worker is
 * restarted). It never decides which puzzle is accepted (D11). */
export const UI_TIMEOUT_MS = 60_000;

export interface WorkerResult {
  json: string;
  millis: number;
}

export class GenerationWorker {
  private worker: Worker | null = null;
  private nextId = 1;
  private pending: {
    id: number;
    resolve: (r: WorkerResult) => void;
    reject: (e: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  } | null = null;

  /** Runs one request; a newer request cancels an unfinished one. */
  run(req: GenerateRequest | CodeRequest, timeoutMs = UI_TIMEOUT_MS): Promise<WorkerResult> {
    if (this.pending) this.abort(new Error('cancelled by a newer request'));
    const worker = this.ensure();
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.abort(new Error(`gave up after ${timeoutMs / 1000} s (UI time limit)`));
      }, timeoutMs);
      this.pending = { id, resolve, reject, timer };
      worker.postMessage({ ...req, id });
    });
  }

  get busy(): boolean {
    return this.pending !== null;
  }

  private ensure(): Worker {
    if (!this.worker) {
      this.worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
      this.worker.onmessage = (e: MessageEvent<WorkerResponse>) => this.settle(e.data);
      this.worker.onerror = (e) => this.abort(new Error(e.message || 'worker failed'));
    }
    return this.worker;
  }

  private settle(res: WorkerResponse): void {
    const p = this.pending;
    if (!p || p.id !== res.id) return;
    clearTimeout(p.timer);
    this.pending = null;
    if (res.ok) p.resolve({ json: res.json, millis: res.millis });
    else p.reject(new Error(res.error));
  }

  /** Rejects the pending request and restarts the worker (a busy wasm call cannot be
   * interrupted any other way). */
  private abort(err: Error): void {
    const p = this.pending;
    this.pending = null;
    this.worker?.terminate();
    this.worker = null;
    if (p) {
      clearTimeout(p.timer);
      p.reject(err);
    }
  }
}
