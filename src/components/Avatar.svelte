<script lang="ts">
  // A channel's profile picture, so names in a list can be told apart at a
  // glance. Until the picture has arrived, or if it fails to load, the
  // channel's initial stands in. Decorative: the name is always beside it.
  let { name, url }: { name: string; url: string | null } = $props();

  // Which address has loaded, and which has failed. Remembering the address,
  // not a yes or no, means a list refresh (which hands this the same address
  // again) changes nothing, while a new address starts over by itself.
  let loadedUrl = $state<string | null>(null);
  let failedUrl = $state<string | null>(null);

  const initial = $derived((name.match(/[\p{L}\p{N}]/u)?.[0] ?? '?').toUpperCase());
</script>

<span class="avatar t-caption t-strong" aria-hidden="true">
  {#if url && failedUrl !== url}
    <img
      class:loaded={loadedUrl === url}
      src={url}
      alt=""
      width="32"
      height="32"
      draggable="false"
      onload={() => (loadedUrl = url)}
      onerror={() => (failedUrl = url)}
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
