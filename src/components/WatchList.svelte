<script lang="ts">
  import { flip } from 'svelte/animate';
  import { app } from '../lib/app.svelte';
  import { MOVE, ms } from '../lib/motion.svelte';
  import StreakMark from './StreakMark.svelte';
  import WatchSlot from './WatchSlot.svelte';

  // The two watch slots and the queue behind them. The tray popover and the
  // window's Watching page both show this, so they cannot drift apart.
  // `--inset` sets the side padding and `--rule-x` where the slot's accent
  // rule sits; the defaults suit the popover.
  const uid = $props.id();

  const rank = $derived(new Map(app.channels.map((c, i) => [c.id, i + 1])));
  // The queue is who is next: live channels waiting for a slot. Offline ones
  // are counted, not listed, so a long list of followed channels stays readable.
  const waiting = $derived(app.queue.filter((c) => c.live));
  // A channel Twitch has not answered about yet is neither live nor offline.
  const checking = $derived(app.queue.filter((c) => !c.checked).length);
  const offline = $derived(app.queue.length - waiting.length - checking);
</script>

<section aria-labelledby="{uid}-watching">
  <h2 id="{uid}-watching" class="t-label section-h">Watching</h2>
  <ul>
    {#each app.slots as channel, i (channel?.id ?? `free-${i}`)}
      <WatchSlot {channel} />
    {/each}
  </ul>
</section>

{#if app.queue.length > 0}
  <section aria-labelledby="{uid}-queue">
    <h2 id="{uid}-queue" class="t-label section-h">Queue</h2>
    {#if waiting.length > 0}
      <ol>
        {#each waiting as channel (channel.id)}
          <li class="row" animate:flip={{ duration: waiting.length > 80 ? 0 : ms(MOVE) }}>
            <span class="rank t-caption t-muted num">{rank.get(channel.id)}</span>
            <span class="name">{channel.name}</span>
            {#if channel.streakDue}<StreakMark />{/if}
            <span class="state t-caption t-strong">Live</span>
          </li>
        {/each}
      </ol>
    {/if}
    {#if offline > 0}
      <p class="row t-caption t-muted">
        {offline}
        {offline === 1 ? 'channel' : 'channels'} offline
      </p>
    {/if}
    {#if checking > 0}
      <p class="row t-caption t-muted" role="status">
        Checking {checking}
        {checking === 1 ? 'channel' : 'channels'}…
      </p>
    {/if}
  </section>
{/if}

<style>
  .section-h {
    padding: var(--space-4) var(--inset, var(--space-4)) var(--space-1);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--control-h);
    padding: 0 var(--inset, var(--space-4));
  }

  /* Wide enough for a three-digit position in a long list. */
  .rank {
    width: var(--space-6);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    flex: 1;
    text-align: right;
  }
</style>
