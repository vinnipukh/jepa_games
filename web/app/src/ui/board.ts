// SVG tubes. Pure rendering: the cells come from the wasm Session.

const SVG = 'http://www.w3.org/2000/svg';
const EMPTY = 255;

/** 16 colors (the largest puzzle code has 16 colors), chosen to stay apart. */
export const PALETTE = [
  '#e6194b', '#3cb44b', '#ffe119', '#4363d8', '#f58231', '#911eb4', '#42d4f4', '#f032e6',
  '#bfef45', '#fabed4', '#469990', '#9a6324', '#800000', '#aaffc3', '#000075', '#a9a9a9',
];

const UNIT_H = 34;
const TUBE_W = 46;
const GAP = 18;
const PAD = 10;

export interface BoardView {
  cells: Uint8Array;
  nTubes: number;
  capacity: number;
  selected: number | null;
  labels: boolean;
}

/** Draws the tubes into `root`; `onTube(i)` fires when tube `i` is clicked. */
export function renderBoard(root: HTMLElement, view: BoardView, onTube: (i: number) => void): void {
  const { cells, nTubes, capacity } = view;
  const perRow = nTubes > 8 ? Math.ceil(nTubes / 2) : nTubes;
  const rows = Math.ceil(nTubes / perRow);
  const tubeH = capacity * UNIT_H + 8;
  const rowH = tubeH + 2 * PAD + 14;
  const width = perRow * (TUBE_W + GAP) + GAP;
  const height = rows * rowH;
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
  svg.setAttribute('class', 'board');
  svg.setAttribute('role', 'group');
  svg.setAttribute('aria-label', 'tubes');
  for (let t = 0; t < nTubes; t++) {
    const row = Math.floor(t / perRow);
    const col = t % perRow;
    const x = GAP + col * (TUBE_W + GAP);
    const lift = view.selected === t ? -12 : 0;
    const y = row * rowH + PAD + 14 + lift;
    const g = document.createElementNS(SVG, 'g');
    g.setAttribute('class', 'tube' + (view.selected === t ? ' selected' : ''));
    g.setAttribute('data-tube', String(t));
    g.setAttribute('tabindex', '0');
    g.setAttribute('role', 'button');
    let filled = 0;
    for (let k = 0; k < capacity; k++) if (cells[t * capacity + k] !== EMPTY) filled++;
    g.setAttribute('aria-label', `tube ${t + 1}, ${filled} of ${capacity} filled`);
    // Hit area, also over the empty part.
    const hit = rect(x - GAP / 2, y - 10, TUBE_W + GAP, tubeH + 20, 'hit');
    g.append(hit);
    for (let k = 0; k < capacity; k++) {
      const c = cells[t * capacity + k];
      const cy = y + tubeH - 4 - (k + 1) * UNIT_H;
      if (c === EMPTY) continue;
      const unit = rect(x + 4, cy, TUBE_W - 8, UNIT_H, 'unit');
      unit.setAttribute('fill', PALETTE[c % PALETTE.length]);
      if (k === 0) unit.setAttribute('rx', '6');
      g.append(unit);
      if (view.labels) {
        const label = document.createElementNS(SVG, 'text');
        label.setAttribute('x', String(x + TUBE_W / 2));
        label.setAttribute('y', String(cy + UNIT_H / 2 + 5));
        label.setAttribute('class', 'unit-label');
        label.textContent = String(c + 1);
        g.append(label);
      }
    }
    // Capacity ticks make partial fills (distributed layout) easy to read.
    for (let k = 1; k < capacity; k++) {
      const ty = y + tubeH - 4 - k * UNIT_H;
      const tick = document.createElementNS(SVG, 'line');
      tick.setAttribute('x1', String(x + 1));
      tick.setAttribute('x2', String(x + 7));
      tick.setAttribute('y1', String(ty));
      tick.setAttribute('y2', String(ty));
      tick.setAttribute('class', 'tick');
      g.append(tick);
    }
    const glass = rect(x, y, TUBE_W, tubeH, 'glass');
    glass.setAttribute('rx', '10');
    g.append(glass);
    const num = document.createElementNS(SVG, 'text');
    num.setAttribute('x', String(x + TUBE_W / 2));
    num.setAttribute('y', String(y - 4));
    num.setAttribute('class', 'tube-num');
    num.textContent = String(t + 1);
    g.append(num);
    g.addEventListener('click', () => onTube(t));
    g.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        onTube(t);
      }
    });
    svg.append(g);
  }
  root.replaceChildren(svg);
}

/** Plays the "invalid target" shake on tube `i`. */
export function shakeTube(root: HTMLElement, i: number): void {
  const g = root.querySelector<SVGGElement>(`g[data-tube="${i}"]`);
  if (!g) return;
  g.classList.remove('shake');
  // Restart the animation.
  void g.getBoundingClientRect();
  g.classList.add('shake');
}

function rect(x: number, y: number, w: number, hgt: number, cls: string): SVGRectElement {
  const r = document.createElementNS(SVG, 'rect');
  r.setAttribute('x', String(x));
  r.setAttribute('y', String(y));
  r.setAttribute('width', String(w));
  r.setAttribute('height', String(hgt));
  r.setAttribute('class', cls);
  return r;
}
