<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { clock, dayLabel, duration, num, startOfDay } from '../lib/format';
  import type { ActivityEvent, EventKind } from '../lib/types';
  import { arrive } from '../lib/motion.svelte';
  import Avatar from '../components/Avatar.svelte';
  import Count from '../components/Count.svelte';
  import Screen from '../components/Screen.svelte';
  import Segmented from '../components/Segmented.svelte';

  type Range = '1' | '7' | '30';
  type Filter = 'all' | EventKind;

  const ranges: { value: Range; label: string }[] = [
    { value: '1', label: 'Today' },
    { value: '7', label: '7 days' },
    { value: '30', label: '30 days' },
  ];
  const filters: { value: Filter; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'watch', label: 'Watching' },
    { value: 'bonus', label: 'Bonuses' },
    { value: 'streak', label: 'Streaks' },
  ];

  // Two ways to read the same period: what happened and when, or what each
  // channel brought in. Picking a channel narrows everything to it.
  type View = 'log' | 'channels';
  const views: { value: View; label: string }[] = [
    { value: 'log', label: 'Timeline' },
    { value: 'channels', label: 'By channel' },
  ];

  let range = $state<Range>('1');
  let filter = $state<Filter>('all');
  let view = $state<View>('log');
  /** The one channel being looked at, or null for all of them. */
  let only = $state<string | null>(null);

  const inRange = $derived.by(() => {
    const from = startOfDay(Date.now()) - (Number(range) - 1) * 86_400_000;
    return app.events.filter((e) => e.at >= from);
  });
  const scoped = $derived(only ? inRange.filter((e) => e.channel === only) : inRange);

  const points = $derived(scoped.reduce((s, e) => s + e.points, 0));
  const bonuses = $derived(scoped.filter((e) => e.kind === 'bonus').length);
  const streaks = $derived(scoped.filter((e) => e.kind === 'streak').length);

  // What each channel brought in over the period, most points first.
  const byChannel = $derived.by(() => {
    const rows = new Map<string, { name: string; points: number; minutes: number; bonuses: number; streaks: number }>();
    for (const e of inRange) {
      let row = rows.get(e.channel);
      if (!row) rows.set(e.channel, (row = { name: e.channel, points: 0, minutes: 0, bonuses: 0, streaks: 0 }));
      row.points += e.points;
      if (e.kind === 'watch') row.minutes += e.amount ?? 0;
      else if (e.kind === 'bonus') row.bonuses += 1;
      else row.streaks += 1;
    }
    return [...rows.values()].toSorted((a, b) => b.points - a.points || a.name.localeCompare(b.name));
  });
  const avatars = $derived(new Map(app.channels.map((c) => [c.name, c.avatar])));

  function pick(name: string) {
    only = name;
    view = 'log';
  }

  const days = $derived.by(() => {
    const shown = scoped
      .filter((e) => filter === 'all' || e.kind === filter)
      .toSorted((a, b) => b.at - a.at);
    const groups: { day: number; total: number; events: ActivityEvent[] }[] = [];
    for (const e of shown) {
      const day = startOfDay(e.at);
      let g = groups.at(-1);
      if (!g || g.day !== day) groups.push((g = { day, total: 0, events: [] }));
      g.events.push(e);
      g.total += e.points;
    }
    return groups;
  });

  function describe(e: ActivityEvent): string {
    if (e.kind === 'watch') return `Watched ${duration((e.amount ?? 0) * 60)}`;
    if (e.kind === 'bonus') return 'Bonus claimed';
    return `Watch streak, day ${e.amount ?? 1}`;
  }
</script>

<Screen title="Activity">
  <Segmented legend="Period" name="activity-range" options={ranges} bind:value={range} />

  <dl class="figures">
    <div>
      <dt class="t-caption t-muted">Points earned</dt>
      <dd class="t-title num"><Count value={points} /></dd>
    </div>
    <div>
      <dt class="t-caption t-muted">Bonuses claimed</dt>
      <dd class="t-title num"><Count value={bonuses} /></dd>
    </div>
    <div>
      <dt class="t-caption t-muted">Streaks kept</dt>
      <dd class="t-title num"><Count value={streaks} /></dd>
    </div>
  </dl>

  <div class="tools">
    <Segmented legend="View" name="activity-view" options={views} bind:value={view} />
    {#if view === 'log'}
      <Segmented legend="Show" name="activity-filter" options={filters} bind:value={filter} />
    {/if}
  </div>

  {#if only}
    <p class="only" in:arrive>
      <span>Showing <span class="t-strong">{only}</span> only.</span>
      <button class="btn" onclick={() => (only = null)}>Show all channels</button>
    </p>
  {/if}

  <!-- A different view, channel or period settles in as one piece. -->
  {#key `${view}|${only}|${range}`}
  <div in:arrive>
  {#if view === 'channels'}
    {#if byChannel.length === 0}
      <p class="t-muted">
        Nothing earned in this period yet. Points show up here once a watched channel pays out.
      </p>
    {:else}
      <div>
        <div class="crow head t-label" aria-hidden="true">
          <span></span>
          <span>Channel</span>
          <span class="n">Watched</span>
          <span class="n">Bonuses</span>
          <span class="n">Streaks</span>
          <span class="n">Points</span>
        </div>
        <ul>
          {#each byChannel as row (row.name)}
            <li>
              <!-- The whole row opens that channel's timeline. -->
              <button class="crow" aria-current={only === row.name ? 'true' : undefined} onclick={() => pick(row.name)}>
                <Avatar name={row.name} url={avatars.get(row.name) ?? null} />
                <span class="t-strong cname">{row.name}</span>
                <span class="n num t-muted">
                  {row.minutes ? duration(row.minutes * 60) : '–'}<span class="vh"> watched</span>
                </span>
                <span class="n num t-muted">{num(row.bonuses)}<span class="vh"> bonuses</span></span>
                <span class="n num t-muted">{num(row.streaks)}<span class="vh"> streaks</span></span>
                <span class="n num t-strong">+{num(row.points)}<span class="vh"> points</span></span>
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  {:else if days.length === 0}
    <p class="t-muted">
      {#if filter === 'all' && !only}
        Nothing earned in this period yet. Points show up here once a watched channel pays out.
      {:else if filter === 'all'}
        Nothing earned on {only} in this period.
      {:else}
        Nothing of this kind in this period.
      {/if}
    </p>
  {:else}
    <div class="log">
      {#each days as g (g.day)}
        <section aria-label={dayLabel(g.day)}>
          <h2 class="t-label day">
            <span>{dayLabel(g.day)}</span>
            <span class="num">+{num(g.total)}</span>
          </h2>
          <ul>
            {#each g.events as e (e.id)}
              <li>
                <span class="t-caption t-muted num">{clock(e.at)}</span>
                <span class="what">
                  <span class="t-strong">{e.channel}</span>
                  <span class="t-muted">{describe(e)}</span>
                </span>
                <span class="t-strong num">+{num(e.points)}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>
  {/if}
  </div>
  {/key}
</Screen>

<style>
  .figures {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    padding: var(--space-3) 0;
    border-block: var(--hairline) solid var(--line);
  }

  .figures div + div {
    padding-left: var(--space-4);
    border-left: var(--hairline) solid var(--line);
  }

  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2) var(--space-3);
  }

  .only {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    border-radius: var(--radius-control);
    background: var(--bg-quiet);
  }

  /* By channel: one line each, the figures in columns that line up. */
  .crow {
    display: grid;
    grid-template-columns: var(--control-h) minmax(0, 1fr) 88px 72px 72px 88px;
    align-items: center;
    column-gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) 0;
    border: 0;
    border-bottom: var(--hairline) solid var(--line);
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  button.crow {
    cursor: pointer;
    transition: background-color var(--motion-quick) var(--ease);
  }

  button.crow:hover,
  button.crow[aria-current] {
    background: var(--bg-quiet);
  }

  .crow.head {
    padding: var(--space-1) 0;
  }

  .cname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .n {
    text-align: right;
  }

  .log {
    display: grid;
    gap: var(--space-4);
  }

  .day {
    display: flex;
    justify-content: space-between;
    padding-bottom: var(--space-1);
    border-bottom: var(--hairline) solid var(--line);
  }

  /* One grid per day so the time column fits any locale's clock format. */
  .log ul {
    display: grid;
    grid-template-columns: max-content 1fr max-content;
    column-gap: var(--space-3);
  }

  .log li {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
    align-items: center;
    min-height: var(--control-h);
    white-space: nowrap;
  }

  .log li > :last-child {
    text-align: right;
  }

  .what {
    display: flex;
    gap: var(--space-2);
    min-width: 0;
    overflow: hidden;
  }
</style>
