// The "new puzzle" form: generator, Turan strategy, layout, params (only supported ones, D3),
// difficulty tier.
import * as wasm from '../wasm';
import { h } from './dom';

export type Generator = 'uniform' | 'turan';
export type LayoutName = 'standard' | 'distributed';
export type TierName = 'any' | 'easy' | 'medium' | 'hard';

export interface GenSettings {
  generator: Generator;
  /** Turan strategy name, e.g. "scramble". */
  strategy: string;
  /** Optional step count for scramble / pour_walk ("" = the generator's default). */
  steps: string;
  layout: LayoutName;
  nColors: number;
  capacity: number;
  nEmpty: number;
  tier: TierName;
}

export const DEFAULT_SETTINGS: GenSettings = {
  generator: 'uniform',
  strategy: 'reverse_search',
  steps: '',
  layout: 'standard',
  nColors: 6,
  capacity: 4,
  nEmpty: 2,
  tier: 'any',
};

interface Row {
  capacity: number;
  n_empty: number;
  min_colors: number;
  max_colors: number;
}

const STEP_STRATEGIES = ['scramble', 'pour_walk'];

/** The strategy spec passed to wasm `generate` (Turan only). */
export function strategySpec(s: GenSettings): string | undefined {
  if (s.generator !== 'turan') return undefined;
  return STEP_STRATEGIES.includes(s.strategy) && s.steps !== ''
    ? `${s.strategy}(steps=${s.steps})`
    : s.strategy;
}

export function wasmParams(s: GenSettings): wasm.Params {
  return new wasm.Params(s.nColors, s.capacity, s.nEmpty);
}

/** Whether the settings name a supported configuration and strategy. */
export function validSettings(s: GenSettings): boolean {
  const p = wasmParams(s);
  try {
    if (!p.is_supported(s.layout)) return false;
  } finally {
    p.free();
  }
  return s.generator === 'uniform' || wasm.strategies(s.layout).includes(s.strategy);
}

function rows(layout: LayoutName): Row[] {
  return JSON.parse(wasm.supported_rows(layout)) as Row[];
}

function options(select: HTMLSelectElement, values: [string, string][], current: string): string {
  select.replaceChildren(...values.map(([v, label]) => h('option', { value: v }, label)));
  const keep = values.some(([v]) => v === current) ? current : values[0][0];
  select.value = keep;
  return keep;
}

export interface Controls {
  element: HTMLElement;
  get(): GenSettings;
  set(s: GenSettings): void;
}

/** Builds the form. `onNew` fires on "New puzzle". */
export function createControls(onNew: (s: GenSettings) => void): Controls {
  const generator = h('select', { id: 'generator', 'aria-label': 'generator' });
  options(generator, [['uniform', 'uniform'], ['turan', 'turan']], 'uniform');
  const strategy = h('select', { id: 'strategy', 'aria-label': 'Turan strategy' });
  const steps = h('input', {
    id: 'steps', type: 'number', min: '1', max: '100000', placeholder: 'default',
    'aria-label': 'strategy steps',
  });
  const layout = h('select', { id: 'layout', 'aria-label': 'layout' });
  options(layout, [['standard', 'standard'], ['distributed', 'distributed']], 'standard');
  const capacity = h('select', { id: 'capacity', 'aria-label': 'capacity' });
  const nEmpty = h('select', { id: 'n_empty', 'aria-label': 'empty tubes' });
  const nColors = h('select', { id: 'n_colors', 'aria-label': 'colors' });
  const tier = h('select', { id: 'tier', 'aria-label': 'difficulty' });
  options(tier, [['any', 'any'], ['easy', 'easy'], ['medium', 'medium'], ['hard', 'hard']], 'any');
  const newButton = h('button', { id: 'new-puzzle', class: 'primary' }, 'New puzzle');

  const strategyField = h('label', {}, 'strategy ', strategy);
  const tierField = h('label', {}, 'difficulty ', tier);
  const stepsField = h('label', {}, 'steps ', steps);

  let current: GenSettings = { ...DEFAULT_SETTINGS };

  /** Re-derives every dependent select from `current`, keeping values that stay valid. */
  function refresh(): void {
    generator.value = current.generator;
    layout.value = current.layout;
    const isTuran = current.generator === 'turan';
    strategyField.hidden = !isTuran;
    current.strategy = options(
      strategy, wasm.strategies(current.layout).map((s) => [s, s.replace('_', ' ')]), current.strategy,
    );
    stepsField.hidden = !isTuran || !STEP_STRATEGIES.includes(current.strategy);
    steps.value = current.steps;
    const rs = rows(current.layout);
    const caps = [...new Set(rs.map((r) => r.capacity))];
    current.capacity = Number(options(capacity, caps.map((c) => [String(c), String(c)]), String(current.capacity)));
    const empties = rs.filter((r) => r.capacity === current.capacity).map((r) => r.n_empty);
    current.nEmpty = Number(options(nEmpty, empties.map((e) => [String(e), String(e)]), String(current.nEmpty)));
    const row = rs.find((r) => r.capacity === current.capacity && r.n_empty === current.nEmpty)!;
    const colors: [string, string][] = [];
    for (let c = row.min_colors; c <= row.max_colors; c++) colors.push([String(c), String(c)]);
    current.nColors = Number(options(nColors, colors, String(current.nColors)));
    // Tier cut points come from uniform's opt_moves distribution (D16, D20).
    tierField.hidden = isTuran;
    if (isTuran) current.tier = 'any';
    tier.value = current.tier;
  }

  function read(): void {
    current = {
      generator: generator.value as Generator,
      strategy: strategy.value,
      steps: steps.value.trim(),
      layout: layout.value as LayoutName,
      nColors: Number(nColors.value),
      capacity: Number(capacity.value),
      nEmpty: Number(nEmpty.value),
      tier: tier.value as TierName,
    };
    refresh();
  }

  for (const el of [generator, strategy, layout, capacity, nEmpty, nColors, tier]) {
    el.addEventListener('change', read);
  }
  steps.addEventListener('change', read);
  newButton.addEventListener('click', () => {
    read();
    onNew(current);
  });

  refresh();
  const element = h(
    'div', { class: 'controls' },
    h('label', {}, 'generator ', generator),
    strategyField,
    stepsField,
    h('label', {}, 'layout ', layout),
    h('label', {}, 'colors ', nColors),
    h('label', {}, 'capacity ', capacity),
    h('label', {}, 'empty ', nEmpty),
    tierField,
    newButton,
  );
  return {
    element,
    get: () => ({ ...current }),
    set(s: GenSettings) {
      current = { ...s };
      refresh();
    },
  };
}
