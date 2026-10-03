import './style.css';
import * as wasm from './wasm';
import { loadWasm } from './wasm';
import { renderBoard, shakeTube } from './ui/board';
import { createControls, strategySpec, type GenSettings } from './ui/controls';
import { copyText, h } from './ui/dom';
import { GenerationWorker } from './generation';
import { renderComplete } from './ui/complete';
import { isHexSeed, parseOpenInput, parseQuery, shareQuery, shareUrl, type UrlTarget } from './url';

/** The page state. All game logic is in `session` (wasm); this only tracks the selection. */
class App {
  puzzle: wasm.Puzzle | null = null;
  /** How the current puzzle is reopened (its share URL). */
  target: UrlTarget | null = null;
  session: wasm.Session | null = null;
  selected: number | null = null;
  labels = false;
  /** While the optimal solution is replayed: a second core Session on the same puzzle. */
  replay: { session: wasm.Session; timer: ReturnType<typeof setInterval>; step: number } | null = null;

  readonly board = h('div', { class: 'board-wrap', id: 'board' });
  readonly status = h('div', { class: 'status', role: 'status' });
  readonly complete = h('div', { id: 'complete-wrap' });
  readonly info = h('div', { class: 'info' });
  readonly moves = h('span', { class: 'moves', id: 'moves' }, '0');
  readonly undo = h('button', { id: 'undo' }, 'Undo');
  readonly restart = h('button', { id: 'restart' }, 'Restart');
  readonly exportButton = h('button', { id: 'export', class: 'small', title: 'Download this play session (pours, undos, restarts) as JSON' }, 'Export trajectory');
  readonly controls = createControls((s) => void this.newPuzzle(s));
  readonly seedInput = h('input', { id: 'open-seed', placeholder: 'seed (hex)', spellcheck: 'false', 'aria-label': 'seed' });
  readonly codeInput = h('input', { id: 'open-code', placeholder: 'puzzle code or share link', spellcheck: 'false', 'aria-label': 'puzzle code' });
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
    this.exportButton.addEventListener('click', () => this.exportTrajectory());
    this.restart.addEventListener('click', () => {
      this.session?.restart();
      this.selected = null;
      this.render();
    });
    const openSeed = h('button', { id: 'open-seed-button' }, 'Open seed');
    openSeed.addEventListener('click', () => void this.openSeed());
    this.seedInput.addEventListener('keydown', (e) => e.key === 'Enter' && void this.openSeed());
    const openCode = h('button', { id: 'open-code-button' }, 'Open');
    openCode.addEventListener('click', () => void this.openInput());
    this.codeInput.addEventListener('keydown', (e) => e.key === 'Enter' && void this.openInput());
    root.replaceChildren(
      h('header', {}, h('h1', {}, 'Water Sort'), this.controls.element),
      h(
        'div', { class: 'open' },
        h('label', {}, this.seedInput, ' ', openSeed, ' with the settings above'),
        h('label', {}, this.codeInput, ' ', openCode),
      ),
      this.info,
      h(
        'div', { class: 'toolbar' },
        h('span', {}, 'Moves: ', this.moves),
        this.undo,
        this.restart,
        h('label', { class: 'small' }, labels, ' color numbers'),
        this.exportButton,
      ),
      this.status,
      this.complete,
      this.board,
    );
  }

  /** Generates in the worker; the page stays responsive and shows a spinner. Without a seed
   * the generator picks a fresh one, which the puzzle records. */
  async newPuzzle(s: GenSettings, seed?: string): Promise<boolean> {
    const ok = await this.request(
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
    if (ok) this.setTarget({ kind: 'seed', settings: s, seed: this.puzzle!.seed! });
    return ok;
  }

  async openCode(code: string): Promise<boolean> {
    const ok = await this.request({ kind: 'code', code }, 'Opening');
    if (ok) this.setTarget({ kind: 'code', code: this.puzzle!.puzzle_code });
    return ok;
  }

  async open(target: UrlTarget): Promise<boolean> {
    if (target.kind === 'code') return this.openCode(target.code);
    this.controls.set(target.settings);
    return this.newPuzzle(target.settings, target.seed);
  }

  async openSeed(): Promise<void> {
    const seed = this.seedInput.value.trim();
    if (!isHexSeed(seed)) {
      this.setStatus('A seed is 1 to 16 hex digits.', true);
      return;
    }
    await this.newPuzzle(this.controls.get(), seed);
  }

  async openInput(): Promise<void> {
    const target = parseOpenInput(this.codeInput.value);
    if (!target) {
      this.setStatus('Paste a puzzle code or a share link.', true);
      return;
    }
    await this.open(target);
  }

  /** Records how to reopen the puzzle and puts it in the address bar. */
  setTarget(t: UrlTarget): void {
    this.target = t;
    history.replaceState(null, '', shareQuery(t));
    this.renderInfo();
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
    this.stopReplay();
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
    if (!s || s.is_solved() || this.replay) return;
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
    if (s.is_solved()) {
      this.setStatus('');
      this.renderComplete();
    }
  }

  renderComplete(): void {
    const p = this.puzzle;
    const s = this.session;
    if (!p || !s || !s.is_solved()) {
      this.complete.replaceChildren();
      return;
    }
    const exportButton = h('button', { id: 'export-complete' }, 'Export trajectory');
    exportButton.addEventListener('click', () => this.exportTrajectory());
    this.complete.replaceChildren(
      renderComplete(p, s, {
        onReplay: () => this.startReplay(),
        onNext: () => void this.newPuzzle(this.controls.get()),
        extra: [exportButton],
      }),
    );
  }

  /** Downloads the session in the Phase 5.3 row format (source "human"). Nothing is uploaded. */
  exportTrajectory(): void {
    const s = this.session;
    const p = this.puzzle;
    if (!s || !p) return;
    const json = s.export_trajectory();
    const id = (JSON.parse(json) as { session_id: string }).session_id;
    const a = h('a', {
      href: URL.createObjectURL(new Blob([json], { type: 'application/json' })),
      download: `water-sort-${p.puzzle_code}-${id.slice(0, 8)}.json`,
    });
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  }

  /** Replays the optimal solution through a fresh core Session, one pour per tick. */
  startReplay(): void {
    const p = this.puzzle;
    const solution = p?.solution;
    if (!p || !solution) return;
    this.stopReplay();
    const session = new wasm.Session(p);
    const n = p.n_tubes;
    let half = false;
    const timer = setInterval(() => {
      const r = this.replay!;
      if (r.step >= solution.length) {
        clearInterval(r.timer);
        this.setStatus(`Optimal solution: ${solution.length} moves.`);
        return;
      }
      const a = solution[r.step];
      if (!half) {
        this.selected = Math.floor(a / n); // show the source tube first
      } else {
        r.session.pour(Math.floor(a / n), a % n);
        this.selected = null;
        r.step++;
        this.setStatus(`Optimal solution: move ${r.step} of ${solution.length}`);
      }
      half = !half;
      this.render();
    }, 350);
    this.replay = { session, timer, step: 0 };
    this.render();
  }

  stopReplay(): void {
    if (!this.replay) return;
    clearInterval(this.replay.timer);
    this.replay.session.free();
    this.replay = null;
    this.selected = null;
  }

  render(): void {
    const s = this.replay?.session ?? this.session;
    if (!s) return;
    renderBoard(
      this.board,
      { cells: s.cells(), nTubes: s.n_tubes, capacity: s.capacity, selected: this.selected, labels: this.labels },
      (i) => this.onTube(i),
    );
    const game = this.session!;
    this.moves.textContent = String(game.moves_counted());
    this.undo.disabled = !game.can_undo() || game.is_solved();
    this.restart.disabled = game.is_solved();
    this.exportButton.disabled = game.n_events() === 0;
    if (!this.replay) this.renderComplete();
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
    if (this.target) {
      const link = shareUrl(this.target);
      const copyLink = h('button', { class: 'small', id: 'copy-link' }, 'copy share link');
      copyLink.addEventListener('click', () => void copyText(link, copyLink));
      parts.push(copyLink);
    }
    const tier = p.tier;
    if (tier) parts.push(h('span', {}, `tier ${tier}`));
    const py = pythonSnippet(p, this.target);
    const copyPy = h('button', { class: 'small', id: 'copy-python' }, 'copy Python');
    copyPy.title = py;
    copyPy.addEventListener('click', () => void copyText(py, copyPy));
    parts.push(copyPy);
    this.info.replaceChildren(...parts);
  }

  setStatus(text: string, error = false): void {
    this.status.textContent = text;
    this.status.classList.toggle('error', error);
  }
}

/** The jepa_water_sort call that gives the same puzzle in Python. */
function pythonSnippet(p: wasm.Puzzle, target: UrlTarget | null): string {
  if (!target || target.kind === 'code' || !p.seed) {
    return `import jepa_water_sort as w\nstate = w.State.from_code("${p.puzzle_code}")`;
  }
  const s = target.settings;
  const args = [`"${s.generator}"`, `(${s.nColors}, ${s.capacity}, ${s.nEmpty})`, `seed=0x${p.seed}`];
  if (s.generator === 'turan') args.push(`strategy="${p.variant}"`);
  if (s.layout !== 'standard') args.push(`layout="${s.layout}"`);
  // Only a tier changes the config in a way that matters (rollouts never change the puzzle).
  if (s.tier !== 'any' && p.config_json) args.push(`config=w.GenConfig.from_json('${p.config_json}')`);
  return `import jepa_water_sort as w\npuzzle = w.generate(${args.join(', ')})\nassert puzzle.puzzle_code == "${p.puzzle_code}"`;
}

async function main(): Promise<void> {
  const root = document.getElementById('app')!;
  root.textContent = 'Loading…';
  await loadWasm();
  const app = new App();
  app.mount(root);
  const target = parseQuery(window.location.search);
  if (!target || !(await app.open(target))) {
    if (target) {
      // Keep the error for the bad link visible while a default puzzle loads.
      const msg = app.status.textContent;
      await app.newPuzzle(app.controls.get());
      app.setStatus(`Could not open the link: ${msg}`, true);
    } else {
      await app.newPuzzle(app.controls.get());
    }
  }
}

void main();
