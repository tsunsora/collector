// Motion in one place. CSS handles what answers the pointer (see base.css);
// the helpers here are for things that enter, leave, change place or count.
// The durations mirror --motion-quick and --motion-move in tokens.css.
import { cubicOut } from 'svelte/easing';
import { MediaQuery } from 'svelte/reactivity';
import type { TransitionConfig } from 'svelte/transition';

export const QUICK = 120;
export const MOVE = 200;
/** A number counting up to its new value. */
export const COUNT = 400;

const reduced = new MediaQuery('prefers-reduced-motion: reduce');
const hold = $state({ on: false });

/** No motion: Windows is set to show fewer animations, or the page is being held still. */
export const still = () => reduced.current || hold.on;

/** A duration, or zero when nothing should move. */
export const ms = (duration: number) => (still() ? 0 : duration);

/**
 * Hold everything still, CSS transitions included. The audit uses this so it
 * measures settled colours and positions, not ones caught mid-fade.
 */
export function holdStill(on: boolean) {
  hold.on = on;
  document.documentElement.toggleAttribute('data-still', on);
}

/** Fade in while settling into place from a few pixels away. */
export function arrive(_node: Element, { x = 0, y = 8, duration = MOVE } = {}): TransitionConfig {
  return {
    duration: ms(duration),
    easing: cubicOut,
    css: (t, u) => `opacity: ${t}; transform: translate(${x * u}px, ${y * u}px)`,
  };
}

/** A plain fade, for things that appear or go without moving. */
export function fade(_node: Element, { duration = MOVE } = {}): TransitionConfig {
  return { duration: ms(duration), easing: cubicOut, css: (t) => `opacity: ${t}` };
}
