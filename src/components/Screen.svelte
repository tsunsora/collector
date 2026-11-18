<script lang="ts">
  import type { Snippet } from 'svelte';
  import { app } from '../lib/app.svelte';
  import ChangedNotice from './ChangedNotice.svelte';
  import Notice from './Notice.svelte';
  import SignInPrompt from './SignInPrompt.svelte';
  import StatusStrip from './StatusStrip.svelte';

  interface Props {
    title: string;
    /** Render the content in every state (Settings must stay reachable). */
    always?: boolean;
    /** A control that sits on the title row. */
    action?: Snippet;
    children: Snippet;
  }
  let { title, always = false, action, children }: Props = $props();
</script>

<!-- Shared frame for window screens: title, then the right thing for the current state. -->
<div class="screen">
  <div class="title">
    <h1 class="t-title">{title}</h1>
    {@render action?.()}
  </div>

  {#if always}
    {@render children()}
  {:else if app.status === 'loading'}
    <div class="skeleton" aria-busy="true">
      <p class="vh" role="status">Loading…</p>
      {#each [72, 56, 64, 48] as w (w)}
        <div class="skel-row"><span class="skel" style:width="{w}%"></span></div>
      {/each}
    </div>
  {:else if app.status === 'loggedOut'}
    <SignInPrompt body="Sign in to Twitch to see and change this." />
  {:else if app.status === 'error'}
    <Notice
      icon="alert"
      title="Twitch didn’t respond"
      body="This couldn’t be loaded. Nothing you set up has been lost."
      action="Try again"
      onaction={() => app.retry()}
    />
  {:else if app.status === 'changed'}
    <ChangedNotice />
  {:else}
    <StatusStrip />
    {@render children()}
  {/if}
</div>

<style>
  .screen {
    display: grid;
    gap: var(--space-4);
  }

  .title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    /* Room for a control without the row changing height when it appears. */
    min-height: var(--control-h);
  }

  .screen :global(.notice) {
    padding: 0;
  }

  .screen :global(.strip) {
    border-radius: var(--radius-control);
    padding: var(--space-2) var(--space-3);
  }

  .skel-row {
    display: flex;
    align-items: center;
    height: var(--space-10);
    border-bottom: var(--hairline) solid var(--line);
  }
</style>
