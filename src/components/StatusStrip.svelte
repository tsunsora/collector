<script lang="ts">
  import { slide } from 'svelte/transition';
  import { app } from '../lib/app.svelte';
  import { MOVE, ms } from '../lib/motion.svelte';
  import Icon from './Icon.svelte';

  // Non-blocking conditions: the last known data stays on screen underneath.
  // At most one of each kind shows: the connection, a channel that is not
  // being watched properly, and an update waiting to go in.

  // The first watched channel with something wrong. Only while collecting:
  // paused or offline, "not getting through" is expected and said elsewhere.
  const troubled = $derived(app.active ? app.slots.find((c) => c?.problem) : undefined);
</script>

{#if app.status === 'rateLimited'}
  <p class="strip" role="status" transition:slide={{ duration: ms(MOVE) }}>
    <Icon name="wait" />
    <span>
      Twitch is rate-limiting requests. Retrying in <span class="num">{app.retryIn}</span> s.
    </span>
  </p>
{:else if app.status === 'offline'}
  <p class="strip" role="status" transition:slide={{ duration: ms(MOVE) }}>
    <Icon name="offline" />
    <span>No connection. Showing the last known state.</span>
  </p>
{/if}

{#if troubled?.problem === 'reports'}
  <p class="strip" role="status" transition:slide={{ duration: ms(MOVE) }}>
    <Icon name="alert" />
    <span>
      Twitch isn’t accepting watch time for {troubled.name}, so points may not arrive there. Still
      trying.
    </span>
  </p>
{:else if troubled?.problem === 'noPoints'}
  <p class="strip" role="status" transition:slide={{ duration: ms(MOVE) }}>
    <Icon name="alert" />
    <span>
      {troubled.name} has been watched for 15 minutes and no points have arrived. Twitch may have
      changed how points are earned.
      {#if app.update.state !== 'off'}Check for an update in Settings.{/if}
    </span>
  </p>
{/if}

{#if app.update.state === 'ready'}
  <p class="strip" role="status" transition:slide={{ duration: ms(MOVE) }}>
    <Icon name="download" />
    <span class="grow">Version {app.update.version} is ready. It installs when you quit.</span>
    <button class="btn" onclick={() => app.installUpdate()}>Restart and update</button>
  </p>
{/if}

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-4);
    background: var(--bg-quiet);
  }

  .grow {
    flex: 1;
  }

  /* A strip with a long sentence wraps; the icon stays with the first line. */
  .strip :global(svg) {
    align-self: flex-start;
    margin-top: var(--space-1);
  }

  .strip :global(.btn svg) {
    margin-top: 0;
  }
</style>
