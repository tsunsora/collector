<script lang="ts">
  import { MediaQuery } from 'svelte/reactivity';
  import { app, PAYOUT_SECONDS } from '../lib/app.svelte';
  import { countdown, duration } from '../lib/format';
  import { fade } from '../lib/motion.svelte';
  import type { Channel } from '../lib/types';
  import Avatar from './Avatar.svelte';
  import Count from './Count.svelte';
  import Icon from './Icon.svelte';
  import StreakMark from './StreakMark.svelte';

  let { channel }: { channel: Channel | null } = $props();

  const reducedMotion = new MediaQuery('prefers-reduced-motion: reduce');

  const remaining = $derived(channel ? (1 - channel.payout) * PAYOUT_SECONDS : 0);
  // Reduced motion: the bar moves in tenths instead of creeping every second.
  const fill = $derived.by(() => {
    const p = channel?.payout ?? 0;
    return (reducedMotion.current ? Math.floor(p * 10) / 10 : p) * 100;
  });
  // Right after a payout the bar snaps back to empty instead of draining.
  const justPaid = $derived((channel?.payout ?? 0) < 2 / PAYOUT_SECONDS);
</script>

{#if channel}
  <li class="slot" class:active={app.active} in:fade>
    <div class="content with-picture">
      <Avatar name={channel.name} url={channel.avatar} />
      <div class="lines">
      <div class="row">
        <span class="name t-strong">{channel.name}</span>
        {#if channel.streakDue}<StreakMark />{/if}
        <span class="end t-strong num">
          {#if channel.pointsKnown}
            <Count value={channel.points} /><span class="vh"> points</span>
          {:else}
            <span aria-hidden="true">–</span><span class="vh">Points not known</span>
          {/if}
        </span>
      </div>
      <div class="row t-caption">
        <span class="time num">Watched {duration(channel.watchSeconds)}</span>
        {#if app.active && channel.problem}
          <!-- In place of the countdown, which would promise points that are not coming. -->
          <span class="end trouble t-strong">
            <Icon name="alert" />
            {channel.problem === 'reports' ? 'Not getting through' : 'No points for 15 min'}
          </span>
        {:else if app.active}
          <span class="end t-muted num">
            {#if channel.payout >= 1}
              +{channel.payoutPoints} due
            {:else}
              +{channel.payoutPoints} in {countdown(remaining)}
            {/if}
          </span>
        {/if}
      </div>
      </div>
    </div>
    <!-- Signature detail: fills toward the next payout. The countdown above is its text equivalent. -->
    <div class="tick" aria-hidden="true">
      <div class="fill" class:snap={justPaid} style:width="{fill}%"></div>
    </div>
  </li>
{:else}
  <li class="slot" in:fade>
    <div class="content">
      <div class="row"><span class="t-muted">Slot free</span></div>
      <div class="row t-caption"><span class="t-muted">No other channel is live.</span></div>
    </div>
    <div class="tick" aria-hidden="true"></div>
  </li>
{/if}

<style>
  .slot {
    position: relative;
  }

  .content {
    display: grid;
    gap: var(--space-1);
    padding: var(--space-3) var(--inset, var(--space-4));
  }

  /* Accent use 1 of 2: the edge rule and running time of an active slot. */
  .slot.active::before {
    content: '';
    position: absolute;
    inset: var(--space-3) auto calc(var(--space-3) + var(--rule)) var(--rule-x, 0);
    width: var(--rule);
    background: var(--accent);
  }

  .with-picture {
    grid-template-columns: auto 1fr;
    align-items: center;
    column-gap: var(--space-3);
  }

  .lines {
    display: grid;
    gap: var(--space-1);
    min-width: 0;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .end {
    flex: 1;
    text-align: right;
    white-space: nowrap;
  }

  /* Going active or neutral (pause, resume, losing the connection) is a fade. */
  .time {
    color: var(--text-muted);
    transition: color var(--motion-move) var(--ease);
  }

  .slot::before {
    transition: background-color var(--motion-move) var(--ease);
  }

  .active .time {
    color: var(--accent);
    font-weight: var(--weight-strong);
  }

  .trouble {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-1);
  }

  .tick {
    height: var(--rule);
    background: var(--line);
  }

  .fill {
    height: 100%;
    background: var(--border);
    transition: width var(--tick) linear;
  }

  .fill.snap {
    transition: none;
  }

  .active .fill {
    background: var(--accent);
  }
</style>
