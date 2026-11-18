<script lang="ts">
  import { tick } from 'svelte';
  import { flip } from 'svelte/animate';
  import { app } from '../lib/app.svelte';
  import { arrive, fade, MOVE, ms } from '../lib/motion.svelte';
  import { num } from '../lib/format';
  import Avatar from '../components/Avatar.svelte';
  import FollowedList from '../components/FollowedList.svelte';
  import Icon from '../components/Icon.svelte';
  import Screen from '../components/Screen.svelte';
  import SearchField from '../components/SearchField.svelte';
  import Segmented from '../components/Segmented.svelte';
  import StreakMark from '../components/StreakMark.svelte';
  import UndoToast from '../components/UndoToast.svelte';

  // Two views: the priority list, and the channels followed on Twitch that
  // could be added to it.
  type View = 'list' | 'followed';
  const views: { value: View; label: string }[] = [
    { value: 'list', label: 'Your list' },
    { value: 'followed', label: 'Followed on Twitch' },
  ];
  let view = $state<View>('list');

  // Searching narrows the list to find a channel. Order is relative to the
  // whole list, so while searching a row offers "move to top" instead of
  // dragging and one-step moves, which would act on rows that are hidden.
  let query = $state('');
  const q = $derived(query.trim().toLowerCase());
  const position = $derived(new Map(app.channels.map((c, i) => [c.id, i + 1])));

  async function toTop(id: string) {
    if (!(await app.moveChannelTo(id, 0))) return;
    await tick();
    document.getElementById(`ch-${id}-top`)?.focus();
  }

  let name = $state('');
  let error = $state<string | null>(null);
  let input = $state<HTMLInputElement>();
  /** The channel is being looked up on Twitch. */
  let adding = $state(false);

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (adding) return;
    adding = true;
    error = await app.addChannel(name);
    adding = false;
    if (!error) name = '';
    input?.focus();
  }

  // Reordering moves the row in the DOM, which can drop focus; put it back on
  // the same control so repeated presses keep working.
  // Reordering moves the row in the DOM, which can drop focus; put it back on
  // the grip so repeated presses keep working.
  async function move(id: string, delta: -1 | 1) {
    if (!(await app.moveChannel(id, delta))) return;
    await tick();
    document.getElementById(`ch-${id}-grip`)?.focus();
  }

  // Removing asks first, in the row itself: the bin turns into "Remove" and
  // "Keep". Undo is still offered afterwards.
  let asking = $state<string | null>(null);

  async function ask(id: string) {
    asking = id;
    await tick();
    document.getElementById(`ch-${id}-confirm`)?.focus();
  }

  async function keep(id: string) {
    asking = null;
    await tick();
    document.getElementById(`ch-${id}-remove`)?.focus();
  }

  async function remove(id: string) {
    asking = null;
    await app.removeChannel(id);
    await tick();
    document.getElementById('undo-remove')?.focus();
  }

  async function undo() {
    const id = app.removed?.id;
    await app.undoRemove();
    await tick();
    document.getElementById(`ch-${id}-remove`)?.focus();
  }

  // The keyboard route to reordering: Up and Down on a focused grip.
  function gripKey(e: KeyboardEvent, id: string) {
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    e.preventDefault();
    move(id, e.key === 'ArrowUp' ? -1 : 1);
  }

  // Pointer reordering. While a row is held, the list shows a local order and
  // the backend is told once, on drop. Keyboard users have the buttons above.
  let list = $state<HTMLOListElement>();
  let drag = $state<{ id: string; order: string[] } | null>(null);

  const rows = $derived(
    drag
      ? drag.order.flatMap((id) => app.channels.find((c) => c.id === id) ?? [])
      : q
        ? app.channels.filter((c) => c.id.includes(q) || c.name.toLowerCase().includes(q))
        : app.channels,
  );

  // Rows glide to their new places after a move or a removal, and part around
  // a row being dragged. Not while searching or in a very long list, where
  // many rows would move at once.
  const glide = $derived(q || rows.length > 80 ? 0 : ms(MOVE));
  // A row fades as it joins or leaves the list. Not in a very long list,
  // where a search would set hundreds fading at once.
  const rowFade = $derived(rows.length > 80 ? 0 : 120);

  // Every row is the same height, measured once when a drag starts. The drop
  // position is worked out from that and the list's own top edge, never from
  // where the rows are drawn, because they are mid-glide while dragging.
  let rowHeight = 0;
  /** How close to the pane's edge the pointer must be for the list to scroll. */
  const EDGE = 48;

  function startDrag(e: PointerEvent, id: string) {
    if (e.button !== 0 || !list?.firstElementChild) return;
    e.preventDefault();
    rowHeight = list.firstElementChild.getBoundingClientRect().height;
    drag = { id, order: app.channels.map((c) => c.id) };
    window.addEventListener('pointermove', dragMove);
    window.addEventListener('pointerup', drop);
    window.addEventListener('pointercancel', cancelDrag);
    window.addEventListener('keydown', dragKey);
  }

  function stopListening() {
    window.removeEventListener('pointermove', dragMove);
    window.removeEventListener('pointerup', drop);
    window.removeEventListener('pointercancel', cancelDrag);
    window.removeEventListener('keydown', dragKey);
  }

  function dragMove(e: PointerEvent) {
    if (!drag || !list || rowHeight <= 0) return;
    // Near the top or bottom of the pane, bring more of the list into reach.
    const pane = list.closest('main');
    if (pane) {
      const { top, bottom } = pane.getBoundingClientRect();
      if (e.clientY < top + EDGE) pane.scrollBy(0, -rowHeight / 2);
      else if (e.clientY > bottom - EDGE) pane.scrollBy(0, rowHeight / 2);
    }
    // The held row goes to whichever slot the pointer is over.
    const slot = Math.floor((e.clientY - list.getBoundingClientRect().top) / rowHeight);
    const to = Math.max(0, Math.min(drag.order.length - 1, slot));
    const from = drag.order.indexOf(drag.id);
    if (to === from) return;
    const order = drag.order.filter((id) => id !== drag!.id);
    order.splice(to, 0, drag.id);
    drag = { id: drag.id, order };
  }

  async function drop() {
    stopListening();
    if (!drag) return;
    await app.moveChannelTo(drag.id, drag.order.indexOf(drag.id));
    drag = null;
  }

  function cancelDrag() {
    stopListening();
    drag = null;
  }

  function dragKey(e: KeyboardEvent) {
    if (e.key === 'Escape') cancelDrag();
  }
</script>

<Screen title="Channels">
  <p class="t-muted intro">
    The two highest channels that are live get watched, and are marked below while they are.
    {#if app.streakFirst}A channel with a streak due jumps ahead until the streak is claimed.{/if}
  </p>

  <Segmented
    legend="Show"
    name="channels-view"
    options={views}
    bind:value={view}
    onchange={(v) => {
      if (v === 'followed') void app.loadFollowed();
    }}
  />

  {#if view === 'followed'}
    <div class="view" in:arrive><FollowedList /></div>
  {:else}
  <div class="view" in:arrive>
  <form onsubmit={add} novalidate>
    <label class="t-label" for="add-name">Add a channel</label>
    <div class="add">
      <input
        id="add-name"
        class="input"
        bind:this={input}
        bind:value={name}
        oninput={() => (error = null)}
        placeholder="Channel name"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        readonly={adding}
        aria-invalid={error ? 'true' : undefined}
        aria-describedby="add-error"
      />
      <button class="btn btn-primary" type="submit" aria-busy={adding}>
        {#if adding}Checking…{:else}<Icon name="plus" />Add{/if}
      </button>
    </div>
    <p id="add-error" class="error t-caption t-strong" role="alert">
      {#if error}<span class="msg" in:fade><Icon name="alert" />{error}</span>{/if}
    </p>
  </form>

  {#if app.channels.length === 0}
    <p class="t-muted empty">
      No channels yet. Add one above, or pick from the channels you follow under “Followed on
      Twitch”.
    </p>
  {:else}
    <SearchField id="list-search" label="Search your list" bind:value={query} />
  {/if}

  {#if app.channels.length > 0 && rows.length === 0}
    <p class="t-muted">No channel in your list matches “{query.trim()}”.</p>
  {:else if app.channels.length > 0}
    <div>
      <div class="row head t-label" aria-hidden="true">
        <span></span>
        <span>#</span>
        <span></span>
        <span>Channel</span>
        <span class="points">Points</span>
        <span class="actions-h"></span>
      </div>
      <ol bind:this={list} class:dragging={drag}>
        {#each rows as channel, i (channel.id)}
          {@const watching = app.active && app.slotIds.includes(channel.id)}
          <li
            class="row"
            class:held={drag?.id === channel.id}
            class:watching
            animate:flip={{ duration: glide }}
            in:fade={{ duration: rowFade }}
            out:fade={{ duration: rowFade }}
          >
            {#if q}
              <span></span>
            {:else}
              <!-- Drag it, or with the keyboard: focus it and press Up or Down. -->
              <button
                id="ch-{channel.id}-grip"
                class="grip"
                aria-label="Reorder {channel.name}. Drag, or press Up or Down."
                title="Drag to reorder"
                onpointerdown={(e) => startDrag(e, channel.id)}
                onkeydown={(e) => gripKey(e, channel.id)}
              >
                <Icon name="grip" />
              </button>
            {/if}
            <span class="t-muted num">{drag ? i + 1 : position.get(channel.id)}</span>
            <Avatar name={channel.name} url={channel.avatar} />
            <span class="who">
              <span class="t-strong name">{channel.name}</span>
              <span class="meta t-caption t-muted">
                {#if watching}
                  <span class="now t-strong">Watching</span>
                {:else}
                  {!channel.checked ? 'Checking…' : channel.live ? 'Live' : 'Offline'}
                {/if}
                {#if channel.streakDue}<StreakMark />{/if}
              </span>
            </span>
            <span class="points num">
              {#if channel.pointsKnown}
                {num(channel.points)}<span class="vh"> points</span>
              {:else}
                <!-- Not a zero: either Twitch has not answered yet, or the channel has no channel points. -->
                <span class="t-muted" aria-hidden="true">–</span>
                <span class="vh">{channel.checked ? 'No channel points' : 'Points not loaded yet'}</span>
              {/if}
            </span>
            <span class="actions">
              <!-- While the question is up, the buttons under it are out of reach. -->
              <span class="buttons" inert={asking === channel.id}>
                {#if q}
                  <button
                    id="ch-{channel.id}-top"
                    class="icon-btn"
                    aria-label="Move {channel.name} to the top"
                    aria-disabled={position.get(channel.id) === 1}
                    title="Move to the top"
                    onclick={() => toTop(channel.id)}
                  >
                    <Icon name="top" />
                  </button>
                {/if}
                <button
                  id="ch-{channel.id}-remove"
                  class="icon-btn"
                  aria-label="Remove {channel.name}"
                  title="Remove"
                  onclick={() => ask(channel.id)}
                >
                  <Icon name="remove" />
                </button>
              </span>
              {#if asking === channel.id}
                <!-- Slides in over the end of the row, so nothing else in it moves. -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span
                  class="confirm"
                  in:arrive={{ x: 12, y: 0, duration: 160 }}
                  out:fade={{ duration: 120 }}
                  onkeydown={(e) => e.key === 'Escape' && keep(channel.id)}
                >
                  <button
                    id="ch-{channel.id}-confirm"
                    class="btn"
                    aria-label="Remove {channel.name} from your list"
                    onclick={() => remove(channel.id)}
                  >
                    Remove
                  </button>
                  <button class="btn btn-ghost" aria-label="Keep {channel.name}" onclick={() => keep(channel.id)}>
                    Keep
                  </button>
                </span>
              {/if}
            </span>
          </li>
        {/each}
      </ol>
    </div>
  {/if}

  <UndoToast onundo={undo} />
  </div>
  {/if}

  <p class="vh" aria-live="polite">{app.announcement}</p>
</Screen>

<style>
  /* The same gap the page has, so a view can settle in as one piece. */
  .view {
    display: grid;
    gap: var(--space-4);
  }

  form {
    display: grid;
    gap: var(--space-1);
  }

  .add {
    display: flex;
    gap: var(--space-2);
  }

  .add .input {
    flex: 1;
  }

  .input[aria-invalid='true'] {
    border-color: var(--text);
    box-shadow: inset 0 0 0 var(--hairline) var(--text);
  }

  .error {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-height: var(--lh-caption);
  }

  .row {
    display: grid;
    grid-template-columns: var(--space-4) var(--space-6) var(--control-h) 1fr auto auto;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: var(--hairline) solid var(--line);
  }

  .grip {
    display: grid;
    place-items: center;
    align-self: stretch;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--border);
    cursor: grab;
    touch-action: none;
  }

  /* Being watched right now: the same rule and colour as a watch slot, hung
     in the margin. Accent use 1 of 2, in its third place. */
  .row {
    position: relative;
  }

  /* Always there and clear, so it can fade in and out as watching starts and stops. */
  .row::before {
    content: '';
    position: absolute;
    inset: var(--space-2) auto var(--space-2) calc(-1 * var(--space-3));
    width: var(--rule);
    background: transparent;
    transition: background-color var(--motion-move) var(--ease);
  }

  .watching::before {
    background: var(--accent);
  }

  .msg {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .now {
    color: var(--accent);
  }

  .grip {
    transition: color var(--motion-quick) var(--ease);
  }

  .grip:hover,
  .held .grip {
    color: var(--text);
  }

  .row {
    transition: background-color var(--motion-quick) var(--ease);
  }

  /* Picked up: raised off the list with a shadow and an outline, above the
     rows that part around it. */
  .held {
    position: relative;
    z-index: 1;
    border-bottom-color: transparent;
    border-radius: var(--radius-control);
    background: var(--bg-pressed);
    box-shadow:
      0 0 0 var(--hairline) var(--border),
      var(--lift);
  }

  .row {
    transition:
      background-color var(--motion-quick) var(--ease),
      box-shadow var(--motion-quick) var(--ease);
  }

  .dragging,
  .dragging .grip {
    cursor: grabbing;
    user-select: none;
  }

  .who {
    display: grid;
    min-width: 0;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .meta :global(.streak) {
    margin-left: var(--space-1);
  }

  .points {
    text-align: right;
  }

  .actions,
  .actions-h {
    display: flex;
    gap: var(--space-1);
    justify-content: flex-end;
    width: calc(2 * var(--icon-btn) + var(--space-1));
  }

  .actions {
    position: relative;
  }

  .buttons {
    display: flex;
    gap: var(--space-1);
  }

  /* Laid over the end of the row rather than pushed into it: the points
     beside it fade out under its leading edge instead of jumping aside. */
  .confirm {
    position: absolute;
    inset: 0 0 0 auto;
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding-left: var(--space-8);
    background: linear-gradient(to right, transparent, var(--bg) var(--space-6));
    white-space: nowrap;
  }

</style>
