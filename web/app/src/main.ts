import './style.css';
import * as wasm from './wasm';
import { loadWasm } from './wasm';
import { renderBoard, shakeTube } from './ui/board';
import { createControls, strategySpec, type GenSettings } from './ui/controls';
import { copyText, h } from './ui/dom';
import { GenerationWorker } from './generation';

/** The page state. All game logic is in `session` (wasm); this only tracks the selection. */
class App {
  puzzle: wasm.Puzzle | null = null;
  session: wasm.Session | null = null;
  selected: number | null = null;
  labels = false;

  readonly board = h('div', { class: 'board-wrap', id: 'board' });
  readonly status = h('div', { class: 'status', role: 'status' });
  readonly info = h('div', { class: 'info' });
  readonly moves = h('span', { class: 'moves', id: 'moves' }, '0');
  readonly undo = h('button', { id: 'undo' }, 'Undo');
  readonly restart = h('button', { id: 'restart' }, 'Restart');
  readonly controls = createControls((s) => void this.newPuzzle(s));
  readonly worker = new GenerationWorker();

  mount(root: HTMLElement): void {
    const labels = h('input', { type: 'checkbox', id: 'labels' });
    labels.addEventListener('change', () => {
      this.labels = labels.checked;
      this.render();
    });
    this.undo.addEventListener('click', () => {
      this.session?.undo();
      this.selected = null;
      this.render();
    });
    this.restart.addEventListener('click', () => {
      this.session?.restart();
      this.selected = null;
      this.render();
    });
    root.replaceChildren(
      h('header', {}, h('h1', {}, 'Water Sort'), this.controls.element),
      this.info,
      h(
        'div', { class: 'toolbar' },
        h('span', {}, 'Moves: ', this.moves),
        this.undo,
        this.restart,
        h('label', { class: 'small' }, labels, ' color numbers'),
      ),
      this.status,
      this.board,
    );
  }

  /** Generates in the worker; the page stays responsive and shows a spinner. */
  async newPuzzle(s: GenSettings, seed?: string): Promise<boolean> {
    return this.request(
      {
        kind: 'generate',
        generator: s.generator,
        params: [s.nColors, s.capacity, s.nEmpty],
        seed,
        strategy: strategySpec(s),
        layout: s.layout,
        tier: s.tier === 'any' ? undefined : s.tier,
      },
      'Generating',
    );
  }

  async request(req: Parameters<GenerationWorker['run']>[0], label: string): Promise<boolean> {
    this.setStatus('');
    this.status.replaceChildren(h('span', { class: 'spinner' }), ` ${label}…`);
    this.setBusy(true);
    try {
      const { json } = await this.worker.run(req);
      this.load(wasm.Puzzle.from_json(json));
      return true;
    } catch (e) {
      const msg = (e as Error).message;
      if (msg !== 'cancelled by a newer request') this.setStatus(`${label} failed: ${msg}`, true);
      return false;
    } finally {
      if (!this.worker.busy) this.setBusy(false);
    }
  }

  setBusy(busy: boolean): void {
    document.body.classList.toggle('busy', busy);
  }

  load(puzzle: wasm.Puzzle): void {
    this.puzzle?.free();
    this.session?.free();
    this.puzzle = puzzle;
    this.session = new wasm.Session(puzzle);
    this.selected = null;
    this.setStatus('');
    this.renderInfo();
    this.render();
  }

  onTube(i: number): void {
    const s = this.session;
    if (!s || s.is_solved()) return;
    if (this.selected === null) {
      const top = s.cells()[i * s.capacity];
      if (top === wasm.empty_cell()) {
        shakeTube(this.board, i); // nothing to pour from an empty tube
        return;
      }
      this.selected = i;
      this.render();
      return;
    }
    const from = this.selected;
    this.selected = null;
    if (from === i) {
      this.render();
      return;
    }
    const units = s.pour(from, i);
    this.render();
    if (units === 0) {
      // Illegal: nothing changes and nothing is counted (core Session).
      shakeTube(this.board, i);
      this.setStatus(s.why_illegal(from, i) ?? '');
    } else {
      this.setStatus('');
    }
    if (s.is_solved()) this.setStatus(`Solved in ${s.moves_counted()} moves!`);
  }

  render(): void {
    const s = this.session;
    if (!s) return;
    renderBoard(
      this.board,
      { cells: s.cells(), nTubes: s.n_tubes, capacity: s.capacity, selected: this.selected, labels: this.labels },
      (i) => this.onTube(i),
    );
    this.moves.textContent = String(s.moves_counted());
    this.undo.disabled = !s.can_undo() || s.is_solved();
    this.restart.disabled = s.is_solved();
  }

  renderInfo(): void {
    const p = this.puzzle;
    if (!p) return;
    const params = p.params;
    const code = p.puzzle_code;
    const copyCode = h('button', { class: 'small', id: 'copy-code' }, 'copy');
    copyCode.addEventListener('click', () => void copyText(code, copyCode));
    const parts: (Node | string)[] = [
      h('span', {}, `${p.generator_id ?? 'puzzle code'}`, p.variant ? ` · ${p.variant}` : ''),
      h('span', {}, `${params.n_colors} colors · capacity ${params.capacity} · ${params.n_empty} empty · ${p.layout}`),
    ];
    params.free();
    const seed = p.seed;
    if (seed) {
      const copySeed = h('button', { class: 'small', id: 'copy-seed' }, 'copy');
      copySeed.addEventListener('click', () => void copyText(seed, copySeed));
      parts.push(h('span', {}, 'seed ', h('code', { id: 'seed' }, seed), ' ', copySeed));
    }
    parts.push(h('span', {}, 'code ', h('code', { id: 'code' }, code), ' ', copyCode));
    this.info.replaceChildren(...parts);
  }

  setStatus(text: string, error = false): void {
    this.status.textContent = text;
    this.status.classList.toggle('error', error);
  }
}

async function main(): Promise<void> {
  const root = document.getElementById('app')!;
  root.textContent = 'Loading…';
  await loadWasm();
  const app = new App();
  app.mount(root);
  await app.newPuzzle(app.controls.get());
}

void main();
