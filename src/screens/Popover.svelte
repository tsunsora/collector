<script lang="ts">
  import { app } from '../lib/app.svelte';
  import Count from '../components/Count.svelte';
  import ChangedNotice from '../components/ChangedNotice.svelte';
  import Icon from '../components/Icon.svelte';
  import Notice from '../components/Notice.svelte';
  import SignInPrompt from '../components/SignInPrompt.svelte';
  import { fade } from '../lib/motion.svelte';
  import StatusStrip from '../components/StatusStrip.svelte';
  import WatchList from '../components/WatchList.svelte';

  // A quick look and a Pause button. Everything else is in the window, which
  // the one button at the bottom opens.
  const empty = $derived(app.channels.length === 0);
</script>

<div class="popover">
  <header>
    <div class="headline">
      {#if app.blocked}
        <h1 class="t-title">Collector</h1>
      {:else}
        <h1 class="total">
          <span class="t-display num">+<Count value={app.earnedToday} /></span>
          <span class="t-muted">points today</span>
        </h1>
      {/if}
      <p class="t-muted" role="status">{app.summary}</p>
    </div>
    {#if app.canPause}
      <button class="btn" onclick={() => app.togglePause()}>
        <Icon name={app.paused ? 'play' : 'pause'} />
        {app.paused ? 'Resume' : 'Pause'}
      </button>
    {/if}
  </header>

  <div class="body" aria-busy={app.status === 'loading'}>
    <!-- Loading, signed out, empty, ready: each fades in when it takes over. -->
    {#key app.blocked ? app.status : empty ? 'empty' : 'ready'}
    <div in:fade>
    {#if app.status === 'loading'}
      <div class="skeleton" aria-hidden="true">
        {#each [0, 1] as i (i)}
          <div class="skel-slot">
            <span class="skel w-name"></span>
            <span class="skel w-sub"></span>
          </div>
        {/each}
        {#each [0, 1, 2] as i (i)}
          <div class="skel-row"><span class="skel w-row"></span></div>
        {/each}
      </div>
    {:else if app.status === 'loggedOut'}
      <SignInPrompt body="Nothing is being collected. Sign in to Twitch to start watching again." />
    {:else if app.status === 'error'}
      <Notice
        icon="alert"
        title="Twitch didn’t respond"
        body="Collecting has stopped and your points are safe. Try again in a moment."
        action="Try again"
        onaction={() => app.retry()}
      />
    {:else if app.status === 'changed'}
      <ChangedNotice />
    {:else if empty}
      <Notice
        icon="channels"
        title="No channels yet"
        body="Open Collector to add the channels you want points on. The top two that are live get watched."
      />
    {:else}
      <StatusStrip />
      <WatchList />
    {/if}
    </div>
    {/key}
  </div>

  <footer>
    <!-- With nothing to watch yet, the window opens where channels are added. -->
    <button class="btn" onclick={() => app.openMain(empty && !app.blocked ? 'channels' : undefined)}>
      Open Collector
    </button>
  </footer>
</div>

<style>
  .popover {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg);
    color: var(--text);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-4);
    border-bottom: var(--hairline) solid var(--line);
  }

  .total {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    font-size: var(--text-body);
    line-height: var(--lh-body);
    font-weight: var(--weight-regular);
  }

  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  footer {
    display: flex;
    padding: var(--space-3) var(--space-4);
    border-top: var(--hairline) solid var(--line);
  }

  footer .btn {
    flex: 1;
  }

  /* Loading */
  .skeleton {
    padding-top: var(--space-4);
  }

  .skel-slot {
    display: grid;
    gap: var(--space-3);
    padding: var(--space-4);
  }

  .skel-row {
    display: flex;
    align-items: center;
    height: var(--control-h);
    padding: 0 var(--space-4);
  }

  .w-name {
    width: 40%;
  }

  .w-sub {
    width: 64%;
  }

  .w-row {
    width: 48%;
  }
</style>
