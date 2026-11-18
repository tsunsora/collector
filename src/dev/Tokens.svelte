<script lang="ts">
  // Token sheet. Values are read back from the live CSS, so this page cannot
  // drift from tokens.css.
  import { onMount } from 'svelte';

  const colours = [
    { token: '--n1', role: 'Window background', pair: null },
    { token: '--n2', role: 'Quiet surface: hover, status strip', pair: null },
    { token: '--n3', role: 'Pressed, selected, skeleton', pair: null },
    { token: '--n4', role: 'Hairline dividers, tick-bar track', pair: null },
    { token: '--n5', role: 'Control borders, disabled icons', pair: '--n1' },
    { token: '--n6', role: 'Secondary text', pair: '--n1' },
    { token: '--n7', role: 'Primary text, primary button fill', pair: '--n1' },
    { token: '--accent', role: 'Active watch slot, streak marker', pair: '--n1' },
  ];
  const modes = ['light', 'dark'] as const;

  const spacing = [
    ['--space-1', 'Icon to label inside a marker'],
    ['--space-2', 'Inline gaps, compact padding'],
    ['--space-3', 'Control padding, slot padding'],
    ['--space-4', 'Popover gutter, section gaps'],
    ['--space-5', 'Reserved'],
    ['--space-6', 'Window gutter, between sections'],
    ['--space-8', 'Control height'],
    ['--space-10', 'Reserved'],
  ];

  const type = [
    { cls: 't-display num', spec: 'Display 28 / 36, 600', sample: '+1,240' },
    { cls: 't-title', spec: 'Title 20 / 28, 600', sample: 'Channels' },
    { cls: 't-strong', spec: 'Body strong 14 / 20, 600', sample: 'CohhCarnage' },
    { cls: '', spec: 'Body 14 / 20, 400', sample: 'Watching 2 of 6 channels' },
    { cls: 't-label', spec: 'Label 12 / 16, 600', sample: 'Queue' },
    { cls: 't-caption num', spec: 'Caption 12 / 16, 400', sample: '+10 in 3:20' },
  ];

  let hex = $state<Record<string, string>>({});
  let px = $state<Record<string, string>>({});
  let root: HTMLElement;

  const toHex = (rgb: string) =>
    '#' +
    (rgb.match(/[\d.]+/g) ?? [])
      .slice(0, 3)
      .map((v) => Math.round(+v).toString(16).padStart(2, '0'))
      .join('');

  function lum(h: string) {
    const [r, g, b] = [1, 3, 5].map((i) => {
      const c = parseInt(h.slice(i, i + 2), 16) / 255;
      return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  }

  function ratio(mode: string, a: string, b: string) {
    const x = hex[`${mode}:${a}`];
    const y = hex[`${mode}:${b}`];
    if (!x || !y) return '';
    const [hi, lo] = [lum(x), lum(y)].sort((p, q) => q - p);
    return ((hi + 0.05) / (lo + 0.05)).toFixed(1) + ':1';
  }

  onMount(() => {
    const h: Record<string, string> = {};
    for (const el of root.querySelectorAll<HTMLElement>('[data-swatch]')) {
      h[el.dataset.swatch!] = toHex(getComputedStyle(el).backgroundColor);
    }
    hex = h;
    const p: Record<string, string> = {};
    for (const el of root.querySelectorAll<HTMLElement>('[data-space]')) {
      p[el.dataset.space!] = getComputedStyle(el).width;
    }
    px = p;
  });
</script>

<div class="sheet" bind:this={root}>
  <header>
    <h1 class="t-title">Tokens</h1>
    <a class="btn" href="#bench">Back to bench</a>
  </header>

  <section aria-labelledby="colour-h">
    <h2 id="colour-h" class="t-label">Colour</h2>
    <div class="modes">
      {#each modes as mode (mode)}
        <div class="mode" style:color-scheme={mode}>
          <h3 class="t-strong">{mode === 'light' ? 'Light' : 'Dark'}</h3>
          <ul>
            {#each colours as c (c.token)}
              <li>
                <span class="swatch" data-swatch="{mode}:{c.token}" style:background="var({c.token})"></span>
                <span class="meta">
                  <span class="t-strong">{c.token}</span>
                  <span class="t-caption t-muted">{c.role}</span>
                </span>
                <span class="values t-caption num">
                  <span>{hex[`${mode}:${c.token}`] ?? ''}</span>
                  {#if c.pair}<span class="t-muted">{ratio(mode, c.token, c.pair)} on n1</span>{/if}
                </span>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    </div>
  </section>

  <section aria-labelledby="type-h">
    <h2 id="type-h" class="t-label">Type: Segoe UI Variable</h2>
    <ul class="type">
      {#each type as t (t.spec)}
        <li>
          <span class="t-caption t-muted">{t.spec}</span>
          <span class={t.cls}>{t.sample}</span>
        </li>
      {/each}
    </ul>
  </section>

  <section aria-labelledby="space-h">
    <h2 id="space-h" class="t-label">Spacing: 4 px grid</h2>
    <ul class="spacing">
      {#each spacing as [token, use] (token)}
        <li>
          <span class="t-strong">{token}</span>
          <span class="t-caption t-muted num">{px[token] ?? ''}</span>
          <span class="bar" data-space={token} style:width="var({token})"></span>
          <span class="t-caption t-muted">{use}</span>
        </li>
      {/each}
    </ul>
  </section>

  <section aria-labelledby="other-h">
    <h2 id="other-h" class="t-label">Shape and motion</h2>
    <ul class="other">
      <li><span class="t-strong">--radius-window</span><span class="t-muted">8 px. Popover and window corners.</span></li>
      <li><span class="t-strong">--radius-control</span><span class="t-muted">4 px. Buttons, inputs, strips.</span></li>
      <li><span class="t-strong">--hairline</span><span class="t-muted">1 px. Dividers and control borders.</span></li>
      <li><span class="t-strong">--rule</span><span class="t-muted">2 px. Slot edge rule, tick bar, focus ring.</span></li>
      <li><span class="t-strong">--motion-quick</span><span class="t-muted">120 ms. A control answering the pointer: hover, press, selection.</span></li>
      <li><span class="t-strong">--motion-move</span><span class="t-muted">200 ms. Things that enter, leave or change place. Numbers count for 400 ms.</span></li>
      <li><span class="t-strong">--ease</span><span class="t-muted">Fast out, gentle stop. The one easing.</span></li>
      <li><span class="t-strong">--tick</span><span class="t-muted">1 s linear. Tick bar. Steps in tenths under reduced motion.</span></li>
      <li><span class="t-strong">--pulse</span><span class="t-muted">1.6 s. Loading placeholders. Still under reduced motion.</span></li>
    </ul>
  </section>
</div>

<style>
  .sheet {
    display: grid;
    gap: var(--space-8);
    max-width: 960px;
    padding: var(--space-6);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  a.btn {
    text-decoration: none;
  }

  section {
    display: grid;
    gap: var(--space-3);
  }

  .modes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: var(--space-4);
  }

  /* Each panel forces its own colour scheme, so both modes show at once. */
  .mode {
    display: grid;
    gap: var(--space-3);
    padding: var(--space-4);
    border: var(--hairline) solid var(--line);
    border-radius: var(--radius-window);
    background: var(--bg);
    color: var(--text);
  }

  .mode ul {
    display: grid;
    gap: var(--space-2);
  }

  .mode li {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: var(--space-3);
  }

  .swatch {
    width: var(--space-10);
    height: var(--space-10);
    border: var(--hairline) solid var(--line);
    border-radius: var(--radius-control);
  }

  .meta,
  .values {
    display: grid;
  }

  .values {
    justify-items: end;
  }

  .type,
  .spacing,
  .other {
    display: grid;
    gap: var(--space-2);
  }

  .type li,
  .other li {
    display: grid;
    grid-template-columns: 200px 1fr;
    align-items: baseline;
    gap: var(--space-4);
  }

  .spacing li {
    display: grid;
    grid-template-columns: 96px 40px 40px 1fr;
    align-items: center;
    gap: var(--space-4);
  }

  .bar {
    height: var(--space-3);
    background: var(--text);
  }
</style>
