// Dev-only DOM audit, exposed as window.__audit(selector). Checks, on what is
// actually rendered:
//   spacing   every padding, margin and gap sits on the 4 px grid
//   contrast  every piece of text is at least 4.5:1 on its real surface
//   names     every focusable control has an accessible name
//   accent    which elements paint with the accent (to confirm it stays reserved)

type RGB = [number, number, number, number];

const parse = (c: string): RGB => {
  const v = (c.match(/[\d.]+/g) ?? []).map(Number);
  return [v[0] ?? 0, v[1] ?? 0, v[2] ?? 0, v[3] ?? 1];
};

const lum = ([r, g, b]: RGB) => {
  const f = (c: number) => {
    c /= 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
};

const ratio = (a: RGB, b: RGB) => {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

function surface(el: Element): RGB {
  for (let e: Element | null = el; e; e = e.parentElement) {
    const bg = parse(getComputedStyle(e).backgroundColor);
    if (bg[3] > 0.99) return bg;
  }
  return [255, 255, 255, 1];
}

const describe = (el: Element) => {
  const cls = typeof el.className === 'string' ? el.className.replace(/\s*svelte-\w+/g, '').trim() : '';
  const text = (el.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 32);
  return `${el.tagName.toLowerCase()}${cls ? '.' + cls.split(/\s+/).join('.') : ''} "${text}"`;
};

const visible = (el: Element) => {
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  return r.width > 1 && r.height > 1 && cs.visibility !== 'hidden' && cs.display !== 'none';
};

const SPACING = [
  'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
  'marginTop', 'marginRight', 'marginBottom', 'marginLeft',
  'rowGap', 'columnGap',
] as const;

export function audit(selector = 'body') {
  const root = document.querySelector(selector);
  if (!root) return { error: `no element for ${selector}` };

  const accentProbe = document.createElement('span');
  accentProbe.style.color = 'var(--accent)';
  root.append(accentProbe);
  const accent = getComputedStyle(accentProbe).color;
  accentProbe.remove();

  const spacing: string[] = [];
  const contrast: string[] = [];
  const names: string[] = [];
  const accentUse = new Set<string>();

  for (const el of [root, ...root.querySelectorAll('*')]) {
    if (el.closest('svg') || el.classList.contains('vh') || !visible(el)) continue;
    const cs = getComputedStyle(el);

    for (const prop of SPACING) {
      const px = parseFloat(cs[prop]);
      // 2 px is the --rule token (the slot reserves room for its tick bar).
      if (Number.isFinite(px) && px !== 0 && Math.abs(px) % 4 !== 0 && Math.abs(px) !== 2) {
        spacing.push(`${describe(el)}: ${prop} ${cs[prop]}`);
      }
    }

    const hasText = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent!.trim());
    if (hasText) {
      const r = ratio(parse(cs.color), surface(el));
      if (r < 4.5) contrast.push(`${describe(el)}: ${r.toFixed(2)}:1`);
    }

    if (cs.color === accent && hasText) accentUse.add(describe(el));
    if (cs.backgroundColor === accent) accentUse.add(describe(el) + ' [fill]');
    if (cs.borderTopColor === accent && parseFloat(cs.borderTopWidth) > 0) accentUse.add(describe(el) + ' [border]');
    for (const pseudo of ['::before', '::after']) {
      const p = getComputedStyle(el, pseudo);
      if (p.content !== 'none' && p.backgroundColor === accent) accentUse.add(describe(el) + ` [${pseudo}]`);
    }

    if (el.matches('button, a[href], input, select, textarea, [tabindex]')) {
      const input = el as HTMLInputElement;
      const name =
        el.getAttribute('aria-label') ||
        (input.labels?.length ? [...input.labels].map((l) => l.textContent).join(' ') : '') ||
        el.textContent;
      if (!name?.trim()) names.push(describe(el));
    }
  }

  return { spacing, contrast, names, accentUse: [...accentUse] };
}

(window as unknown as { __audit: typeof audit }).__audit = audit;
