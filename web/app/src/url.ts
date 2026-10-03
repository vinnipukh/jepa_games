// Shareable URLs:
//   ?code=<puzzle code>
//   ?gen=uniform&seed=<hex>&c=6&k=4&e=2[&layout=distributed][&tier=hard]
//   ?gen=turan&seed=<hex>&c=6&k=4&e=2&strategy=scramble[&steps=40][&layout=distributed]
import { DEFAULT_SETTINGS, type GenSettings, type Generator, type LayoutName, type TierName } from './ui/controls';

export type UrlTarget =
  | { kind: 'code'; code: string }
  | { kind: 'seed'; settings: GenSettings; seed: string };

const HEX = /^(0x)?[0-9a-fA-F]{1,16}$/;

function int(v: string | null, fallback: number): number {
  const n = v === null ? NaN : Number(v);
  return Number.isInteger(n) && n > 0 ? n : fallback;
}

/** Reads a puzzle target from a query string; `null` if it names none. Invalid values are left
 * for wasm to reject with a message. */
export function parseQuery(search: string): UrlTarget | null {
  const q = new URLSearchParams(search);
  const code = q.get('code');
  if (code) return { kind: 'code', code: code.trim() };
  const gen = q.get('gen');
  const seed = q.get('seed');
  if (!gen || !seed) return null;
  const strategy = q.get('strategy') ?? DEFAULT_SETTINGS.strategy;
  const settings: GenSettings = {
    generator: gen as Generator,
    strategy,
    steps: q.get('steps') ?? '',
    layout: (q.get('layout') ?? 'standard') as LayoutName,
    nColors: int(q.get('c'), DEFAULT_SETTINGS.nColors),
    capacity: int(q.get('k'), DEFAULT_SETTINGS.capacity),
    nEmpty: int(q.get('e'), DEFAULT_SETTINGS.nEmpty),
    tier: (q.get('tier') ?? 'any') as TierName,
  };
  return { kind: 'seed', settings, seed: seed.trim() };
}

/** Accepts a puzzle code, or a pasted share URL / query string. */
export function parseOpenInput(text: string): UrlTarget | null {
  const t = text.trim();
  if (t === '') return null;
  const q = t.indexOf('?');
  if (q >= 0) return parseQuery(t.slice(q));
  return { kind: 'code', code: t };
}

export function isHexSeed(text: string): boolean {
  return HEX.test(text.trim());
}

/** The query string that reopens a puzzle: the generator + seed form when known, else the code. */
export function shareQuery(target: UrlTarget): string {
  const q = new URLSearchParams();
  if (target.kind === 'code') {
    q.set('code', target.code);
    return `?${q}`;
  }
  const s = target.settings;
  q.set('gen', s.generator);
  q.set('seed', target.seed);
  q.set('c', String(s.nColors));
  q.set('k', String(s.capacity));
  q.set('e', String(s.nEmpty));
  if (s.layout !== 'standard') q.set('layout', s.layout);
  if (s.generator === 'turan') {
    q.set('strategy', s.strategy);
    if (s.steps !== '') q.set('steps', s.steps);
  } else if (s.tier !== 'any') {
    q.set('tier', s.tier);
  }
  return `?${q}`;
}

export function shareUrl(target: UrlTarget): string {
  const u = new URL(window.location.href);
  u.search = shareQuery(target);
  u.hash = '';
  return u.toString();
}
