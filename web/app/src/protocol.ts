// Messages between the page and the generation worker.

export interface GenerateRequest {
  kind: 'generate';
  generator: string;
  params: [number, number, number];
  /** Hex; none = a fresh seed from the generator's seed source. */
  seed?: string;
  strategy?: string;
  layout: string;
  tier?: string;
}

export interface CodeRequest {
  kind: 'code';
  code: string;
}

export type WorkerRequest = (GenerateRequest | CodeRequest) & { id: number };

/** `json` is `Puzzle.to_json()`; the page reads it back with `Puzzle.from_json`. */
export type WorkerResponse =
  | { id: number; ok: true; json: string; millis: number }
  | { id: number; ok: false; error: string };
