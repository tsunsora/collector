<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { clock } from '../lib/format';
  import { fade } from '../lib/motion.svelte';
  import type { ThemeMode } from '../lib/types';
  import Screen from '../components/Screen.svelte';
  import Segmented from '../components/Segmented.svelte';
  import SignInPrompt from '../components/SignInPrompt.svelte';

  // If the backend refuses a change (for example Windows blocks the startup
  // entry), put the checkbox back to what is actually set.
  async function setting(
    box: HTMLInputElement,
    set: (enabled: boolean) => Promise<boolean>,
    current: () => boolean,
  ) {
    await set.call(app, box.checked);
    box.checked = current();
  }

  // One line saying where updating stands, and the one thing to do about it.
  const updateLine = $derived.by(() => {
    const u = app.update;
    switch (u.state) {
      case 'off':
        return 'Updates aren’t set up in this build.';
      case 'checking':
        return 'Checking for a newer version…';
      case 'available':
        return `Version ${u.version} is available.`;
      case 'downloading':
        return `Downloading version ${u.version}…`;
      case 'ready':
        return `Version ${u.version} is ready. It installs when you quit.`;
      case 'failed':
        return u.message ?? 'The update check didn’t work.';
      default:
        return u.checkedAt ? `Up to date. Checked at ${clock(u.checkedAt)}.` : 'Not checked yet.';
    }
  });
  const updating = $derived(app.update.state === 'checking' || app.update.state === 'downloading');

  const themes: { value: ThemeMode; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ];
</script>

<Screen title="Settings" always>
  <section aria-labelledby="set-theme">
    <h2 id="set-theme" class="t-label">Appearance</h2>
    <div class="row">
      <div>
        <p>Theme</p>
        <p class="t-caption t-muted">System follows the Windows light or dark setting.</p>
      </div>
      <Segmented
        legend="Theme"
        name="settings-theme"
        options={themes}
        value={app.theme}
        onchange={(t) => app.setTheme(t)}
      />
    </div>
  </section>

  <section aria-labelledby="set-watching">
    <h2 id="set-watching" class="t-label">Watching</h2>
    <label class="row">
      <span>
        <span class="line">Put streaks first</span>
        <span class="line t-caption t-muted">
          A live channel with a streak due takes a watch slot ahead of your list order, for up to
          seven minutes. Off, the two highest live channels are always the ones watched.
        </span>
      </span>
      <input
        class="check"
        type="checkbox"
        checked={app.streakFirst}
        onchange={(e) => setting(e.currentTarget, app.setStreakFirst, () => app.streakFirst)}
      />
    </label>
  </section>

  <section aria-labelledby="set-startup">
    <h2 id="set-startup" class="t-label">Startup</h2>
    <label class="row">
      <span>
        <span class="line">Open when I sign in to Windows</span>
        <span class="line t-caption t-muted">Starts in the tray, without opening this window.</span>
      </span>
      <input
        class="check"
        type="checkbox"
        checked={app.openAtLogin}
        onchange={(e) => setting(e.currentTarget, app.setOpenAtLogin, () => app.openAtLogin)}
      />
    </label>
    <label class="row">
      <span>
        <span class="line">Start paused</span>
        <span class="line t-caption t-muted">Nothing is watched until you press Resume.</span>
      </span>
      <input
        class="check"
        type="checkbox"
        checked={app.startPaused}
        onchange={(e) => setting(e.currentTarget, app.setStartPaused, () => app.startPaused)}
      />
    </label>
  </section>

  <section aria-labelledby="set-update">
    <h2 id="set-update" class="t-label">Updates</h2>
    <div class="row">
      <div>
        <p>Version <span class="num">{app.version}</span></p>
        <p class="t-caption t-muted" role="status">{updateLine}</p>
        {#if app.update.notes && (app.update.state === 'available' || app.update.state === 'ready')}
          <p class="t-caption t-muted">{app.update.notes}</p>
        {/if}
      </div>
      {#key app.update.state}
      <span class="swap" in:fade>
      {#if app.update.state === 'ready'}
        <button class="btn btn-primary" onclick={() => app.installUpdate()}>Restart and update</button>
      {:else if app.update.state === 'available'}
        <button class="btn btn-primary" onclick={() => app.downloadUpdate()}>Download</button>
      {:else if app.update.state !== 'off'}
        <button class="btn" aria-busy={updating} aria-disabled={updating} onclick={() => !updating && app.checkUpdate()}>
          {app.update.state === 'checking'
            ? 'Checking…'
            : app.update.state === 'downloading'
              ? 'Downloading…'
              : app.update.state === 'failed'
                ? 'Try again'
                : 'Check now'}
        </button>
      {/if}
      </span>
      {/key}
    </div>
    {#if app.update.state !== 'off'}
      <label class="row">
        <span>
          <span class="line">Update automatically</span>
          <span class="line t-caption t-muted">
            Downloads a new version when there is one. It installs when you quit.
          </span>
        </span>
        <input
          class="check"
          type="checkbox"
          checked={app.autoUpdate}
          onchange={(e) => setting(e.currentTarget, app.setAutoUpdate, () => app.autoUpdate)}
        />
      </label>
    {/if}
  </section>

  <section aria-labelledby="set-help">
    <h2 id="set-help" class="t-label">Help and quitting</h2>
    <div class="row">
      <div>
        <p>Welcome tour</p>
        <p class="t-caption t-muted">The five steps shown the first time the app opened.</p>
      </div>
      <button class="btn" onclick={() => app.startTour()}>Show again</button>
    </div>
    <div class="row">
      <div>
        <p>Log file</p>
        <p class="t-caption t-muted">
          What the app has done and anything that went wrong. Worth sending with a problem report.
        </p>
      </div>
      <button class="btn" onclick={() => app.showLog()}>Show in folder</button>
    </div>
    <div class="row">
      <div>
        <p>Quit Collector</p>
        <p class="t-caption t-muted">
          Stops collecting. Closing this window only hides it; the app stays in the tray.
        </p>
      </div>
      <button class="btn" onclick={() => app.quit()}>Quit</button>
    </div>
  </section>

  <section aria-labelledby="set-account">
    <h2 id="set-account" class="t-label">Twitch account</h2>
    {#key app.signInCode ? 'code' : app.account ? 'in' : app.status === 'loading' ? 'loading' : 'out'}
    <div class="row" in:fade>
      {#if app.signInCode}
        <SignInPrompt body="" />
      {:else if app.status === 'loading' && !app.account}
        <p class="t-muted" role="status">Signing in…</p>
      {:else if app.account}
        <div>
          <p>Signed in as <span class="t-strong">{app.account}</span></p>
          <p class="t-caption t-muted">Signing out stops collecting on every channel.</p>
        </div>
        <button class="btn" onclick={() => app.signOut()}>Sign out</button>
      {:else}
        <div>
          <p>Not signed in</p>
          <p class="t-caption t-muted">Nothing is collected until you sign in.</p>
        </div>
        <button class="btn btn-primary" onclick={() => app.signIn()}>Sign in to Twitch</button>
      {/if}
    </div>
    {/key}
  </section>
</Screen>

<style>
  section {
    display: grid;
  }

  section + section {
    margin-top: var(--space-2);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: var(--space-10);
    padding: var(--space-3) 0;
    border-bottom: var(--hairline) solid var(--line);
  }

  .swap {
    display: flex;
    flex: none;
  }

  label.row {
    cursor: pointer;
  }

  .line {
    display: block;
  }
</style>
