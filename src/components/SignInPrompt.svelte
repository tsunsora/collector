<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { clock } from '../lib/format';
  import { arrive } from '../lib/motion.svelte';
  import Icon from './Icon.svelte';
  import Notice from './Notice.svelte';

  /** What being signed out means on the screen this sits on. */
  let { body }: { body: string } = $props();

  let copied = $state(false);

  async function copy() {
    if (!app.signInCode) return;
    try {
      await navigator.clipboard.writeText(app.signInCode.userCode);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      // Clipboard unavailable: the code is selectable, so it can still be copied by hand.
    }
  }
</script>

<!-- Signed out, in its two steps: offer to sign in, then show the code Twitch issued. -->
{#if app.signInCode}
  <div class="notice" in:arrive|global>
    <span class="icon"><Icon name="account" /></span>
    <div class="copy">
      <p class="t-strong">Enter this code at Twitch</p>
      <p class="code">
        <span class="t-display value">{app.signInCode.userCode}</span>
        <button class="btn btn-ghost" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button>
      </p>
      <p class="t-muted">
        Open Twitch, enter the code and approve it. This updates on its own once you have. The code
        works until <span class="num">{clock(app.signInCode.expiresAt)}</span>.
      </p>
      <div class="actions">
        <button class="btn btn-primary" onclick={() => app.openSignInPage()}>Open Twitch</button>
        <button class="btn" onclick={() => app.cancelSignIn()}>Cancel</button>
      </div>
      <p class="vh" role="status">{copied ? 'Code copied.' : ''}</p>
    </div>
  </div>
{:else}
  <Notice
    icon="account"
    title="You’re signed out"
    {body}
    action="Sign in"
    onaction={() => app.signIn()}
  />
{/if}

<style>
  .notice {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2);
    padding: var(--space-4);
  }

  .icon {
    display: grid;
    place-items: center;
    height: var(--lh-body);
  }

  .copy {
    display: grid;
    gap: var(--space-2);
  }

  .code {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  /* Wide tracking so the eight characters are easy to read across to a browser. */
  .value {
    letter-spacing: 0.08em;
    user-select: all;
  }

  .actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }
</style>
