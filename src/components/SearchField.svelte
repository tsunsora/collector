<script lang="ts">
  import Icon from './Icon.svelte';

  interface Props {
    id: string;
    /** Names the field for screen readers and is shown as its placeholder. */
    label: string;
    value: string;
  }
  let { id, label, value = $bindable() }: Props = $props();
</script>

<!-- type="search": Escape clears it, and the browser adds a clear button. -->
<div class="search">
  <Icon name="search" />
  <label class="vh" for={id}>{label}</label>
  <input {id} class="input" type="search" bind:value placeholder={label} autocomplete="off" spellcheck="false" />
</div>

<style>
  .search {
    position: relative;
    display: flex;
    flex: 1;
    min-width: 0;
    color: var(--text-muted);
  }

  .search :global(svg) {
    position: absolute;
    top: var(--space-2);
    left: var(--space-2);
    pointer-events: none;
  }

  .input {
    flex: 1;
    padding-left: var(--space-8);
  }
</style>
