<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { arrive } from '../lib/motion.svelte';
  import type { Section } from '../lib/types';
  import Icon, { type IconName } from '../components/Icon.svelte';
  import WelcomeTour from '../components/WelcomeTour.svelte';
  import Activity from './Activity.svelte';
  import Channels from './Channels.svelte';
  import Settings from './Settings.svelte';
  import Watching from './Watching.svelte';

  const items: { section: Section; label: string; icon: IconName }[] = [
    { section: 'watching', label: 'Watching', icon: 'watching' },
    { section: 'channels', label: 'Channels', icon: 'channels' },
    { section: 'activity', label: 'Activity', icon: 'activity' },
    { section: 'settings', label: 'Settings', icon: 'settings' },
  ];
</script>

<div class="shell">
<!-- While the tour is up, the window behind it is a picture: nothing in it takes focus or a press. -->
<div class="window" inert={app.touring}>
  <nav aria-label="Sections">
    <p class="app t-strong">Collector</p>
    <ul>
      {#each items as item (item.section)}
        <li>
          <button
            class="item"
            aria-current={app.section === item.section ? 'page' : undefined}
            onclick={() => (app.section = item.section)}
          >
            <Icon name={item.icon} />
            {item.label}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <main>
    <!-- A page settles in when you switch to it. The one you left just goes. -->
    {#key app.section}
      <div in:arrive>
        {#if app.section === 'watching'}
          <Watching />
        {:else if app.section === 'channels'}
          <Channels />
        {:else if app.section === 'activity'}
          <Activity />
        {:else}
          <Settings />
        {/if}
      </div>
    {/key}
  </main>
</div>
{#if app.touring}<WelcomeTour />{/if}
</div>

<style>
  .shell {
    position: relative;
    height: 100%;
  }

  .window {
    display: grid;
    grid-template-columns: 168px 1fr;
    height: 100%;
    background: var(--bg);
    color: var(--text);
  }

  nav {
    display: grid;
    align-content: start;
    gap: var(--space-3);
    padding: var(--space-6) var(--space-2);
    border-right: var(--hairline) solid var(--line);
  }

  .app {
    padding: 0 var(--space-2);
    line-height: var(--lh-title);
  }

  ul {
    display: grid;
    gap: var(--space-1);
  }

  .item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: var(--control-h);
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--text-muted);
    text-align: left;
    cursor: pointer;
    transition:
      background-color var(--motion-quick) var(--ease),
      color var(--motion-quick) var(--ease);
  }

  .item:hover {
    background: var(--bg-quiet);
  }

  .item[aria-current='page'] {
    background: var(--bg-pressed);
    color: var(--text);
    font-weight: var(--weight-strong);
  }

  /* Positioned, so absolutely placed children (the screen-reader-only text)
     scroll with this pane instead of stretching the window behind it into a
     second scroll bar. */
  main {
    position: relative;
    min-width: 0;
    overflow-y: auto;
    padding: var(--space-6);
  }
</style>
