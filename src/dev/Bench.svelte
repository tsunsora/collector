<script lang="ts">
  // Preview bench: both windows at their real sizes, plus switches for every
  // state. Development only; never shipped as a window.
  import { app } from '../lib/app.svelte';
  import { longList, seedChannels } from '../lib/mock';
  import { mock } from '../lib/mockBackend';
  import type { Status, ThemeMode, UpdateState } from '../lib/types';
  import Segmented from '../components/Segmented.svelte';
  import MainWindow from '../screens/MainWindow.svelte';
  import Popover from '../screens/Popover.svelte';

  type BenchState = Status | 'code';
  const states: { value: BenchState; label: string }[] = [
    { value: 'ok', label: 'Connected' },
    { value: 'loading', label: 'Loading' },
    { value: 'offline', label: 'Offline' },
    { value: 'loggedOut', label: 'Signed out' },
    { value: 'code', label: 'Sign-in code' },
    { value: 'error', label: 'Error' },
    { value: 'rateLimited', label: 'Rate-limited' },
    { value: 'changed', label: 'Twitch changed' },
  ];
  const updates: { value: UpdateState; label: string }[] = [
    { value: 'off', label: 'Not set up' },
    { value: 'idle', label: 'Up to date' },
    { value: 'available', label: 'Available' },
    { value: 'downloading', label: 'Downloading' },
    { value: 'ready', label: 'Ready' },
    { value: 'failed', label: 'Failed' },
  ];

  function setUpdate(state: UpdateState) {
    const found = state === 'available' || state === 'downloading' || state === 'ready';
    mock.setUpdate({
      state,
      version: found ? '0.5.1' : null,
      notes: found ? 'Faster first balances, and a warning when points stop arriving.' : null,
      checkedAt: state === 'off' ? null : Date.now(),
      message: state === 'failed' ? 'Couldn’t reach the update server. Check your connection and try again.' : null,
    });
  }
  const themes: { value: ThemeMode; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ];
  type Data = 'seed' | 'noneLive' | 'checking' | 'trouble' | 'long' | 'empty';
  const datasets: { value: Data; label: string }[] = [
    { value: 'seed', label: 'Six channels' },
    { value: 'noneLive', label: 'None live' },
    { value: 'checking', label: 'Being checked' },
    { value: 'trouble', label: 'Slot trouble' },
    { value: 'long', label: 'Sixty channels' },
    { value: 'empty', label: 'No channels' },
  ];

  let data = $state<Data>('seed');

  function setState(s: BenchState) {
    if (s === 'code') void mock.signIn();
    else mock.setStatus(s);
  }

  function setData(d: Data) {
    const seed = seedChannels();
    mock.setChannels(
      d === 'empty'
        ? []
        : d === 'noneLive'
          ? seed.map((c) => ({ ...c, live: false }))
          : d === 'checking'
            ? seed.map((c, i) => (i < 2 ? c : { ...c, live: false, points: 0, checked: false, pointsKnown: false }))
            : d === 'trouble'
              ? seed.map((c) => (c.id === 'lirik' ? { ...c, problem: 'noPoints' as const, payout: 1 } : c.id === 'cohhcarnage' ? { ...c, problem: 'reports' as const } : c))
            : d === 'long'
              ? longList()
              : seed,
    );
  }
</script>

<div class="bench">
  <header>
    <h1 class="t-label">Preview bench. Not part of the app.</h1>
    <div class="controls">
      <Segmented legend="State" name="bench-state" options={states} value={app.signInCode ? 'code' : app.status} onchange={setState} />
      <Segmented legend="Data" name="bench-data" options={datasets} bind:value={data} onchange={setData} />
      <Segmented legend="Theme" name="bench-theme" options={themes} value={app.theme} onchange={(t) => app.setTheme(t)} />
      <Segmented legend="Update" name="bench-update" options={updates} value={app.update.state} onchange={setUpdate} />
      <button class="btn" onclick={() => app.startTour()}>Welcome tour</button>
      <a class="btn" href="#tokens">Token sheet</a>
    </div>
  </header>

  <div class="frames">
    <figure>
      <figcaption class="t-label">Tray popover, 360 × 520</figcaption>
      <div class="frame popover"><Popover /></div>
    </figure>
    <figure>
      <figcaption class="t-label">Window, 720 × 520</figcaption>
      <div class="frame window"><MainWindow /></div>
    </figure>
  </div>
</div>

<style>
  .bench {
    min-height: 100%;
    background: var(--bg-quiet);
  }

  header {
    display: grid;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-6);
    border-bottom: var(--hairline) solid var(--line);
    background: var(--bg);
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }

  a.btn {
    text-decoration: none;
  }

  .frames {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-6);
    padding: var(--space-6);
  }

  figure {
    display: grid;
    gap: var(--space-2);
    margin: 0;
  }

  .frame {
    border: var(--hairline) solid var(--border);
    border-radius: var(--radius-window);
    overflow: hidden;
    background: var(--bg);
  }

  .frame.popover {
    width: 360px;
    height: 520px;
  }

  .frame.window {
    width: 720px;
    height: 520px;
  }
</style>
