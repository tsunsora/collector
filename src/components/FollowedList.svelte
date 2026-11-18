<script lang="ts">
  import { tick } from 'svelte';
  import { flip } from 'svelte/animate';
  import { app } from '../lib/app.svelte';
  import { MOVE, ms } from '../lib/motion.svelte';
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import Notice from './Notice.svelte';
  import SearchField from './SearchField.svelte';

  // The channels the user follows on Twitch that are not in the list yet,
  // each with an Add button, and one button to add everything shown.
  let query = $state('');
  /** "Add all" has been pressed once and is asking to be confirmed. */
  let confirming = $state(false);

  const listed = $derived(new Set(app.channels.map((c) => c.id)));
  const available = $derived((app.followed ?? []).filter((f) => !listed.has(f.login)));
  const q = $derived(query.trim().toLowerCase());
  const shown = $derived(
    q ? available.filter((f) => f.login.includes(q) || f.name.toLowerCase().includes(q)) : available,
  );
  const noun = $derived(shown.length === 1 ? 'channel' : 'channels');
  // Rows close up over an added one. Not while searching or in a very long
  // list, where many rows would be moving at once.
  const glide = $derived(q || shown.length > 80 ? 0 : ms(MOVE));

  // A different search is a different set to add; ask again.
  $effect(() => {
    void q;
    confirming = false;
  });

  const focus = (id: string) => tick().then(() => document.getElementById(id)?.focus());

  // The added row leaves the list. Focus goes to its neighbour so the next
  // one can be added with the same key.
  async function add(login: string) {
    const i = shown.findIndex((f) => f.login === login);
    const next = shown[i + 1]?.login ?? shown[i - 1]?.login;
    if (await app.addFollowed([login])) focus(next ? `fol-${next}` : 'followed-search');
  }

  async function addAll() {
    await app.addFollowed(shown.map((f) => f.login));
    confirming = false;
    focus('followed-search');
  }
</script>

{#if app.followed === null && app.followedError}
  <Notice
    icon="alert"
    title="Followed channels didn’t load"
    body={app.followedError}
    action="Try again"
    onaction={() => app.loadFollowed(true)}
  />
{:else if app.followed === null}
  <div aria-busy="true">
    <p class="vh" role="status">Loading the channels you follow…</p>
    {#each [56, 40, 64, 48] as w (w)}
      <div class="skel-row"><span class="skel" style:width="{w}%"></span></div>
    {/each}
  </div>
{:else if app.followed.length === 0}
  <p class="t-muted">You don’t follow any channels on Twitch yet.</p>
{:else if available.length === 0}
  <p class="t-muted">Every channel you follow is already in your list.</p>
{:else}
  <div class="tools">
    <SearchField id="followed-search" label="Search followed channels" bind:value={query} />
    <button class="btn btn-ghost" aria-busy={app.followedLoading} onclick={() => app.loadFollowed(true)}>
      <Icon name="refresh" />
      Refresh
    </button>
  </div>
  {#if app.followedError}
    <p class="error t-caption t-strong" role="alert"><Icon name="alert" />{app.followedError}</p>
  {/if}

  {#if shown.length === 0}
    <p class="t-muted">No followed channel matches “{query.trim()}”.</p>
  {:else}
    <div>
      <div class="bulk">
        {#if confirming}
          <span>Add {shown.length} {noun} to the end of your list?</span>
          <span class="buttons">
            <button id="followed-confirm" class="btn btn-primary" onclick={addAll}>Add {shown.length}</button>
            <button
              class="btn"
              onclick={() => {
                confirming = false;
                focus('followed-all');
              }}
            >
              Cancel
            </button>
          </span>
        {:else}
          <span class="t-muted">
            {shown.length}
            {noun}
            {q ? 'found' : 'not in your list'}
          </span>
          {#if shown.length > 1}
            <button
              id="followed-all"
              class="btn"
              onclick={() => {
                confirming = true;
                focus('followed-confirm');
              }}
            >
              Add all
            </button>
          {/if}
        {/if}
      </div>
      <ul>
        {#each shown as f (f.login)}
          <li class="row" animate:flip={{ duration: glide }}>
            <Avatar name={f.name} url={f.avatar} />
            <span class="who">
              <span class="t-strong name">{f.name}</span>
              {#if f.name.toLowerCase() !== f.login}
                <span class="t-caption t-muted">{f.login}</span>
              {/if}
            </span>
            <button id="fol-{f.login}" class="btn" aria-label="Add {f.name}" onclick={() => add(f.login)}>
              <Icon name="plus" />
              Add
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
{/if}

<style>
  .tools {
    display: flex;
    gap: var(--space-2);
  }

  .error {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .bulk,
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: var(--hairline) solid var(--line);
  }

  /* Holds its height whether or not a button is in it. */
  .bulk {
    min-height: calc(var(--control-h) + 2 * var(--space-2));
  }

  .buttons {
    display: flex;
    gap: var(--space-2);
  }

  .who {
    display: grid;
    flex: 1;
    min-width: 0;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .skel-row {
    display: flex;
    align-items: center;
    height: var(--space-10);
    border-bottom: var(--hairline) solid var(--line);
  }
</style>
