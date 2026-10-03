/** Creates an element with attributes and children. */
export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | boolean | undefined> = {},
  ...children: (Node | string | null | undefined)[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === false) continue;
    if (k === 'class') el.className = String(v);
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children) {
    if (c !== null && c !== undefined) el.append(c);
  }
  return el;
}

/** Copies text to the clipboard and flashes the button. */
export async function copyText(text: string, button: HTMLElement): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    flash(button, 'copied');
  } catch {
    flash(button, 'copy failed');
  }
}

function flash(button: HTMLElement, label: string): void {
  const old = button.textContent;
  button.textContent = label;
  setTimeout(() => (button.textContent = old), 1200);
}
