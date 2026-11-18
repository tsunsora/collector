// Checks every foreground/surface pairing the UI uses against WCAG contrast,
// in both modes, straight from src/styles/tokens.css. Exits 1 on any failure.
import { readFileSync } from 'node:fs';

const css = readFileSync(new URL('../src/styles/tokens.css', import.meta.url), 'utf8');
const tokens = {};
for (const m of css.matchAll(/--([\w-]+):\s*light-dark\((#[0-9a-f]{6}),\s*(#[0-9a-f]{6})\)/gi)) {
  tokens[m[1]] = { light: m[2], dark: m[3] };
}

const lum = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const ratio = (a, b) => {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

// [foreground, surface, minimum, where it is used]
const pairs = [
  ['n7', 'n1', 4.5, 'primary text on window'],
  ['n7', 'n2', 4.5, 'primary text on quiet surface / hover'],
  ['n7', 'n3', 4.5, 'primary text on pressed / selected'],
  ['n6', 'n1', 4.5, 'secondary text on window'],
  ['n6', 'n2', 4.5, 'secondary text on quiet surface / hover'],
  ['n6', 'n3', 4.5, 'secondary text on pressed / selected'],
  ['n1', 'n7', 4.5, 'primary button label'],
  ['accent', 'n1', 4.5, 'accent text on window'],
  ['accent', 'n2', 4.5, 'accent text on quiet surface'],
  ['accent', 'n4', 3, 'tick-bar fill against its track'],
  ['n5', 'n1', 3, 'control border on window'],
  ['n5', 'n2', 3, 'control border on quiet surface'],
  ['n7', 'n1', 3, 'focus ring on window'],
];

let failed = 0;
for (const mode of ['light', 'dark']) {
  console.log(`\n${mode}`);
  for (const [fg, bg, min, use] of pairs) {
    const r = ratio(tokens[fg][mode], tokens[bg][mode]);
    const ok = r >= min;
    if (!ok) failed++;
    console.log(
      `  ${ok ? 'pass' : 'FAIL'}  ${r.toFixed(2).padStart(5)}  (min ${min})  ${fg} on ${bg}  ${use}`,
    );
  }
}
process.exit(failed ? 1 : 0);
