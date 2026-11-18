<script lang="ts" module>
  // One set: 16 px grid, 1.5 px stroke, round caps and joins, currentColor.
  const paths = {
    pause: '<path d="M5.5 3.5v9M10.5 3.5v9"/>',
    play: '<path d="M5 3.5l7 4.5-7 4.5z"/>',
    plus: '<path d="M8 3v10M3 8h10"/>',
    up: '<path d="M8 12.5v-9M4 7.5l4-4 4 4"/>',
    down: '<path d="M8 3.5v9M4 8.5l4 4 4-4"/>',
    remove: '<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.5 8.5h6l.5-8.5"/>',
    grip: '<path d="M6 4h.01M10 4h.01M6 8h.01M10 8h.01M6 12h.01M10 12h.01"/>',
    undo: '<path d="M4 6.5h5.5a3 3 0 0 1 0 6H7M6 4L3.5 6.5 6 9"/>',
    channels: '<path d="M6 4h7M6 8h7M6 12h7M3 4h.01M3 8h.01M3 12h.01"/>',
    activity: '<path d="M2 8h2.5l2-4.5 3 9 2-4.5H14"/>',
    settings:
      '<path d="M3 5h4.5M11.5 5H13M3 11h1.5M8.5 11H13"/><circle cx="9.5" cy="5" r="2"/><circle cx="6.5" cy="11" r="2"/>',
    alert: '<path d="M8 2.5l6 10.5H2zM8 6.5v3M8 11.2h.01"/>',
    offline:
      '<path d="M2.5 6.5a8 8 0 0 1 11 0M4.8 9a5 5 0 0 1 6.4 0M8 11.8h.01M3 2.5l10 11"/>',
    account:
      '<circle cx="8" cy="5.5" r="2.5"/><path d="M3 13.5c.6-2.5 2.6-4 5-4s4.4 1.5 5 4"/>',
    wait: '<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3l2 1.5"/>',
    watching:
      '<path d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/>',
    next: '<path d="M6 3.5L10.5 8 6 12.5"/>',
    check: '<path d="M3.5 8.5l3 3 6-7"/>',
    download: '<path d="M8 2.5v8M4.5 7.5L8 11l3.5-3.5M3 13.5h10"/>',
    close: '<path d="M4 4l8 8M12 4l-8 8"/>',
    search: '<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5l3 3"/>',
    top: '<path d="M3.5 3h9M8 13V6.5M4.5 10L8 6.5l3.5 3.5"/>',
    refresh: '<path d="M13 8a5 5 0 1 1-1.5-3.6M13 3v2.5h-2.5"/>',
  } as const;

  export type IconName = keyof typeof paths;
</script>

<script lang="ts">
  let { name }: { name: IconName } = $props();
</script>

<svg
  width="16"
  height="16"
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {@html paths[name]}
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
