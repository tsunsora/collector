<script lang="ts">
  import { app } from '../lib/app.svelte';
  import Count from '../components/Count.svelte';
  import Icon from '../components/Icon.svelte';
  import Notice from '../components/Notice.svelte';
  import Screen from '../components/Screen.svelte';
  import WatchList from '../components/WatchList.svelte';
</script>

<!-- The window's copy of what the tray popover shows: today's total, Pause, slots and queue. -->
<Screen title="Watching">
  {#snippet action()}
    {#if app.canPause}
      <button class="btn" onclick={() => app.togglePause()}>
        <Icon name={app.paused ? 'play' : 'pause'} />
        {app.paused ? 'Resume' : 'Pause'}
      </button>
    {/if}
  {/snippet}

  {#if app.channels.length === 0}
    <Notice
      icon="channels"
      title="No channels yet"
      body="Add the channels you want points on. The top two that are live get watched."
      action="Add a channel"
      onaction={() => (app.section = 'channels')}
    />
  {:else}
    <div>
      <p class="total">
        <span class="t-display num">+<Count value={app.earnedToday} /></span>
        <span class="t-muted">points today</span>
      </p>
      <p class="t-muted" role="status">{app.summary}</p>
    </div>
    <div class="list"><WatchList /></div>
  {/if}
</Screen>

<style>
  .total {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }

  /* Text lines up with the page title; the accent rule hangs in the margin. */
  .list {
    --inset: 0;
    --rule-x: calc(-1 * var(--space-3));
  }
</style>
