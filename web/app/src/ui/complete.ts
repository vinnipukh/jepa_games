// The completion panel: moves, optimum, stars and their thresholds (all computed in wasm).
import * as wasm from '../wasm';
import { h } from './dom';

export interface CompleteActions {
  onReplay: () => void;
  onNext: () => void;
  extra?: HTMLElement[];
}

function starRow(n: number): HTMLElement {
  const el = h('div', { class: 'stars', id: 'stars', 'aria-label': `${n} of 5 stars` });
  for (let i = 1; i <= 5; i++) el.append(h('span', { class: i <= n ? 'on' : 'off' }, '★'));
  return el;
}

export function renderComplete(puzzle: wasm.Puzzle, session: wasm.Session, actions: CompleteActions): HTMLElement {
  const moves = session.moves_counted();
  const opt = puzzle.opt_moves;
  const stars = session.stars();
  const replay = h('button', { id: 'replay' }, 'Show optimal solution');
  replay.disabled = puzzle.solution === undefined;
  replay.addEventListener('click', actions.onReplay);
  const next = h('button', { id: 'next', class: 'primary' }, 'Next puzzle');
  next.addEventListener('click', actions.onNext);
  const facts = h(
    'table', {},
    h('tr', {}, h('th', {}, 'Your moves'), h('td', { id: 'player-moves' }, String(moves))),
    h('tr', {}, h('th', {}, 'Optimal'), h('td', { id: 'opt-moves' }, opt === undefined ? 'unknown' : String(opt))),
  );
  const children: (Node | string)[] = [h('h2', {}, 'Solved!')];
  if (stars !== undefined && opt !== undefined) {
    children.push(starRow(stars));
    const [s5, s4, s3, s2] = wasm.star_moves(opt);
    const t = h('table', { class: 'thresholds' }, h('tr', {}, h('th', {}, 'Stars'), h('th', {}, 'Moves')));
    const rows: [string, string][] = [
      ['★★★★★', `${s5}`],
      ['★★★★', s4 > s5 ? `${s5 + 1}–${s4}` : `${s4}`],
      ['★★★', `${s4 + 1}–${s3}`],
      ['★★', `${s3 + 1}–${s2}`],
      ['★', `${s2 + 1}+`],
    ];
    for (const [label, range] of rows) t.append(h('tr', {}, h('td', {}, label), h('td', {}, range)));
    children.push(facts, t);
  } else {
    children.push(facts, h('p', {}, 'No stars: the optimum of this puzzle is unknown (the solver gave up).'));
  }
  children.push(h('div', { class: 'actions' }, replay, ...(actions.extra ?? []), next));
  return h('section', { class: 'complete', id: 'complete' }, ...children);
}
