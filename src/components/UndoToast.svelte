<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { arrive, fade, ms } from '../lib/motion.svelte';
  import Icon from './Icon.svelte';

  // The toast that follows removing a channel: what went, a way back, and a
  // way to wave it off. It floats at the bottom of the pane, stays for a few
  // seconds with a line that runs down, and waits while the pointer or the
  // keyboard is on it.
  let { onundo }: { onundo: () => void } = $props();

  /** How long it stays. The line along its bottom edge runs for the same time. */
  const LIFE = 8000;
  let paused = $state(false);

  const dismiss = () => (app.removed = null);

  // With motion on, the running line ends the toast when it runs out, so the
  // two cannot disagree. With motion off there is no line, so a timer does it.
  $effect(() => {
    if (!app.removed || paused || ms(LIFE) > 0) return;
    const timer = setTimeout(dismiss, LIFE);
    return () => clearTimeout(timer);
  });
</script>

{#if app.removed}
  {#key app.removed.id}
    <!-- Hovering or focusing it only holds it open; the buttons do the work. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="toast"
      class:paused
      role="status"
      in:arrive={{ y: 12 }}
      out:fade
      onmouseenter={() => (paused = true)}
      onmouseleave={() => (paused = false)}
      onfocusin={() => (paused = true)}
      onfocusout={() => (paused = false)}
      onkeydown={(e) => e.key === 'Escape' && dismiss()}
    >
      <Icon name="remove" />
      <span class="text"><span class="t-strong">{app.removed.name}</span> removed</span>
      <button id="undo-remove" class="undo" onclick={onundo}>
        <Icon name="undo" />
        Undo
      </button>
      <button class="close" aria-label="Dismiss" title="Dismiss" onclick={dismiss}>
        <Icon name="close" />
      </button>
      <span class="life" style:animation-duration="{LIFE}ms" onanimationend={dismiss}></span>
    </div>
  {/key}
{/if}

<style>
  /* Inverted, so it reads as sitting above the page rather than in it. */
  .toast {
    position: sticky;
    bottom: var(--space-4);
    z-index: 2;
    justify-self: center;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    max-width: 100%;
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    border-radius: var(--radius-window);
    overflow: hidden;
    background: var(--text);
    color: var(--bg);
    box-shadow: var(--lift);
  }

  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .undo,
  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    flex: none;
    height: var(--icon-btn);
    border: 0;
    border-radius: var(--radius-control);
    font: inherit;
    cursor: pointer;
    transition:
      background-color var(--motion-quick) var(--ease),
      transform var(--motion-quick) var(--ease);
  }

  .undo:active,
  .close:active {
    transform: scale(0.97);
  }

  /* The way back is the one bright thing on it. */
  .undo {
    padding: 0 var(--space-3);
    background: var(--bg);
    color: var(--text);
    font-weight: var(--weight-strong);
  }

  .undo:hover {
    background: var(--bg-pressed);
  }

  .close {
    width: var(--icon-btn);
    padding: 0;
    background: transparent;
    color: var(--bg);
  }

  .close:hover {
    background: color-mix(in srgb, var(--bg) 20%, transparent);
  }

  /* On the dark toast the usual focus ring would not show. */
  .undo:focus-visible,
  .close:focus-visible {
    outline: var(--rule) solid var(--bg);
    outline-offset: var(--rule);
  }

  /* Runs down along the bottom edge: how long before it goes by itself. */
  .life {
    position: absolute;
    inset: auto 0 0 0;
    height: var(--rule);
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    transform-origin: left;
    animation: run-down linear forwards;
  }

  .paused .life {
    animation-play-state: paused;
  }

  @keyframes run-down {
    to {
      transform: scaleX(0);
    }
  }
</style>
