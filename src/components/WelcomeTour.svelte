<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { app } from '../lib/app.svelte';
  import { arrive, fade } from '../lib/motion.svelte';
  import type { Channel, Section } from '../lib/types';
  import Icon from './Icon.svelte';
  import SignInPrompt from './SignInPrompt.svelte';
  import WatchSlot from './WatchSlot.svelte';

  // The first-run welcome: a panel over the window that walks through signing
  // in, adding channels, what watching does and where the app lives. The page
  // behind it switches to match each stop. Skip or Escape ends it at any stop,
  // the dots jump between stops, and Settings can show it again.
  type StopId = 'welcome' | 'signin' | 'channels' | 'watching' | 'tray';
  const stops: { id: StopId; section: Section | null; title: string; line: string }[] = [
    {
      id: 'welcome',
      section: null,
      title: 'Collector',
      line: 'Collects channel points on the Twitch channels you choose, while it sits in the tray.',
    },
    {
      id: 'signin',
      section: 'settings',
      title: 'Sign in to Twitch',
      line: 'Twitch gives you a code to approve in your browser. The app never sees your password.',
    },
    {
      id: 'channels',
      section: 'channels',
      title: 'Add your channels',
      line: 'Start with the channels you follow. Channels is where you add others by name and put them in order.',
    },
    {
      id: 'watching',
      section: 'watching',
      title: 'It watches the top two',
      line: 'The two highest channels that are live get watched. Bonuses are claimed and streaks collected as they come up.',
    },
    {
      id: 'tray',
      section: 'watching',
      title: 'It stays in the tray',
      line: 'Closing this window leaves it collecting. Click the tray icon for a quick look. Windows may keep the icon behind the ^ arrow on the taskbar.',
    },
  ];

  const uid = $props.id();
  const stop = $derived(stops[app.tourStop] ?? stops[0]);
  const last = $derived(app.tourStop >= stops.length - 1);

  // The window behind shows the page the stop is about.
  $effect(() => {
    const section = stop.section;
    if (section) app.section = section;
  });

  // Followed channels, offered on the channels stop once signed in.
  const listed = $derived(new Set(app.channels.map((c) => c.id)));
  const toAdd = $derived((app.followed ?? []).filter((f) => !listed.has(f.login)));
  let added = $state<number | null>(null);

  $effect(() => {
    if (stop.id === 'channels' && app.account) untrack(() => void app.loadFollowed());
  });

  async function addFollowed() {
    const count = toAdd.length;
    if (await app.addFollowed(toAdd.map((f) => f.login))) added = count;
  }

  // A stop whose own action has not been taken is passed with a quiet
  // "Not now", so the action stays the main button on the panel.
  const notNow = $derived(
    (stop.id === 'signin' && !app.account) ||
      (stop.id === 'channels' && added === null && toAdd.length > 0),
  );

  // The slot picture is the user's own first slot when there is one.
  const sample: Channel = {
    id: 'sample',
    name: 'Lirik',
    live: true,
    points: 9870,
    watchSeconds: 780,
    payout: 0.62,
    payoutPoints: 10,
    streakDue: true,
    checked: true,
    pointsKnown: true,
    problem: null,
    avatar: null,
  };
  const pictured = $derived(app.slots.find((c) => c !== null) ?? sample);

  async function setOpenAtLogin(box: HTMLInputElement) {
    await app.setOpenAtLogin(box.checked);
    box.checked = app.openAtLogin;
  }

  let forward = $state<HTMLButtonElement>();
  onMount(() => forward?.focus());

  // Which way the tour is moving, so a stop slides in from the side it comes from.
  let direction = $state(1);

  function go(to: number) {
    direction = to < app.tourStop ? -1 : 1;
    app.tourStop = to;
  }

  function next() {
    if (last) app.endTour();
    else go(app.tourStop + 1);
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') app.endTour();
  }
</script>

<div class="scrim" transition:fade|global>
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby="{uid}-title"
    tabindex="-1"
    onkeydown={key}
    in:arrive|global={{ y: 12 }}
  >
    <div class="page" aria-live="polite">
      {#key stop.id}
      <div class="stop" in:arrive={{ x: direction * 24, y: 0 }}>
      {#if stop.id === 'welcome'}
        <img class="mark" src="/app-icon.png" alt="" width="64" height="64" />
      {:else if stop.id === 'watching'}
        <div class="picture">
          <ul aria-hidden={pictured === sample}>
            <WatchSlot channel={pictured} />
          </ul>
          {#if pictured === sample}<p class="t-caption t-muted">An example</p>{/if}
        </div>
      {/if}

      <h2 id="{uid}-title" class="t-title">{stop.title}</h2>
      <p class="line t-muted">{stop.line}</p>

      {#if stop.id === 'signin'}
        {#if app.account}
          <p class="done"><Icon name="check" />Signed in as <span class="t-strong">{app.account}</span></p>
        {:else if app.signInCode}
          <div class="embed"><SignInPrompt body="" /></div>
        {:else if app.status === 'loading'}
          <p class="t-muted" role="status">Checking your sign-in…</p>
        {:else if app.status === 'loggedOut'}
          <button class="btn btn-primary" onclick={() => app.signIn()}>Sign in to Twitch</button>
        {:else}
          <p class="t-muted">Twitch can’t be reached right now. You can sign in later from Settings.</p>
        {/if}
      {:else if stop.id === 'channels'}
        {#if added !== null}
          <p class="done"><Icon name="check" />{added} {added === 1 ? 'channel' : 'channels'} added</p>
        {:else if !app.account}
          <p class="t-muted">Sign in to see the channels you follow.</p>
        {:else if app.followed === null && app.followedError}
          <p class="t-muted" role="alert">{app.followedError}</p>
          <button class="btn" onclick={() => app.loadFollowed(true)}>Try again</button>
        {:else if app.followed === null}
          <p class="t-muted" role="status">Looking up the channels you follow…</p>
        {:else if toAdd.length > 0}
          <button class="btn btn-primary" onclick={addFollowed}>
            Add {toAdd.length} followed {toAdd.length === 1 ? 'channel' : 'channels'}
          </button>
        {:else if app.followed.length === 0}
          <p class="t-muted">You don’t follow any channels on Twitch yet.</p>
        {:else}
          <p class="done"><Icon name="check" />Every channel you follow is in your list</p>
        {/if}
      {:else if stop.id === 'tray'}
        <label class="option">
          <input
            class="check"
            type="checkbox"
            checked={app.openAtLogin}
            onchange={(e) => setOpenAtLogin(e.currentTarget)}
          />
          Open when I sign in to Windows
        </label>
      {/if}
      </div>
      {/key}
    </div>

    <div class="foot">
      <span class="side">
        {#if !last}
          <button class="btn btn-ghost" onclick={() => app.endTour()}>Skip</button>
        {/if}
      </span>
      <ol class="dots">
        {#each stops as s, i (s.id)}
          <li>
            <button
              class="dot"
              aria-label="Step {i + 1} of {stops.length}: {s.title}"
              aria-current={i === app.tourStop ? 'step' : undefined}
              onclick={() => go(i)}
            >
              <span></span>
            </button>
          </li>
        {/each}
      </ol>
      <span class="side end">
        <button bind:this={forward} class="btn" class:btn-primary={!notNow} onclick={next}>
          {last ? 'Done' : notNow ? 'Not now' : 'Next'}
          {#if !last && !notNow}<Icon name="next" />{/if}
        </button>
      </span>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: var(--space-6);
    background: var(--scrim);
  }

  .panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    max-width: calc(var(--space-10) * 11);
    max-height: 100%;
    border: var(--hairline) solid var(--border);
    border-radius: var(--radius-window);
    background: var(--bg);
    color: var(--text);
  }

  /* A floor on the height, so the buttons below stay put from stop to stop. */
  .page {
    display: grid;
    align-content: center;
    min-height: calc(var(--space-10) * 6);
    padding: var(--space-8) var(--space-6) var(--space-4);
    overflow: hidden auto;
  }

  .stop {
    display: grid;
    justify-items: center;
    gap: var(--space-3);
    text-align: center;
  }

  .mark {
    display: block;
  }

  .picture {
    display: grid;
    gap: var(--space-1);
    width: 100%;
    max-width: calc(var(--space-10) * 8);
    text-align: left;
  }

  .picture p {
    text-align: center;
  }

  .line {
    max-width: 44ch;
  }

  .done,
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .option {
    min-height: var(--control-h);
    cursor: pointer;
  }

  .embed {
    text-align: left;
  }

  .foot {
    display: flex;
    align-items: center;
    padding: var(--space-3) var(--space-4) var(--space-4);
  }

  /* Equal sides keep the dots in the middle whatever the buttons say. */
  .side {
    display: flex;
    flex: 1;
  }

  .side.end {
    justify-content: flex-end;
  }

  .dots {
    display: flex;
  }

  /* A pill for the current stop, dots for the rest. Neutral: the accent stays reserved. */
  .dot {
    display: block;
    padding: var(--space-2) var(--space-1);
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    cursor: pointer;
  }

  .dot span {
    display: block;
    width: var(--space-2);
    height: var(--space-2);
    border-radius: var(--radius-control);
    background: var(--border);
    /* The pill slides along as the tour moves, rather than jumping from dot to dot. */
    transition:
      width var(--motion-move) var(--ease),
      background-color var(--motion-move) var(--ease);
  }

  .dot[aria-current] span {
    width: var(--space-6);
    background: var(--text);
  }
</style>
