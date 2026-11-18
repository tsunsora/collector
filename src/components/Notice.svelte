<script lang="ts">
  import { arrive } from '../lib/motion.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    icon: IconName;
    title: string;
    body: string;
    action?: string;
    onaction?: () => void;
  }
  let { icon, title, body, action, onaction }: Props = $props();
</script>

<!-- A blocking state: says what happened, what it means, and the one way forward. -->
<div class="notice" in:arrive|global>
  <span class="icon"><Icon name={icon} /></span>
  <div class="copy">
    <p class="t-strong">{title}</p>
    <p class="t-muted">{body}</p>
    {#if action}
      <button class="btn btn-primary" onclick={onaction}>{action}</button>
    {/if}
  </div>
</div>

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

  .btn {
    margin-top: var(--space-3);
  }
</style>
