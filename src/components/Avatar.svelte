<script lang="ts">
  // A channel's profile picture, so names in a list can be told apart at a
  // glance. Until Twitch has given one, or if it fails to load, the channel's
  // initial stands in. Decorative: the name is always beside it.
  let { name, url }: { name: string; url: string | null } = $props();

  let failed = $state(false);
  let loaded = $state(false);
  // A different picture gets a fresh chance to load.
  $effect(() => {
    void url;
    failed = false;
    loaded = false;
  });

  const initial = $derived((name.match(/[\p{L}\p{N}]/u)?.[0] ?? '?').toUpperCase());
</script>

<span class="avatar t-caption t-strong" aria-hidden="true">
  {#if url && !failed}
    <img
      class:loaded
      src={url}
      alt=""
      width="32"
      height="32"
      draggable="false"
      onload={() => (loaded = true)}
      onerror={() => (failed = true)}
    />
  {:else}
    {initial}
  {/if}
</span>

<style>
  .avatar {
    display: grid;
    place-items: center;
    flex: none;
    width: var(--control-h);
    height: var(--control-h);
    border-radius: 50%;
    overflow: hidden;
    background: var(--bg-pressed);
    color: var(--text-muted);
    user-select: none;
  }

  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    /* Fades in once it has arrived, instead of popping in part-drawn. */
    opacity: 0;
    transition: opacity var(--motion-move) var(--ease);
  }

  img.loaded {
    opacity: 1;
  }
</style>
