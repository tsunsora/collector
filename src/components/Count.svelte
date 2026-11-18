<script lang="ts">
  import { cubicOut } from 'svelte/easing';
  import { Tween } from 'svelte/motion';
  import { num } from '../lib/format';
  import { COUNT, ms } from '../lib/motion.svelte';

  // A number that counts to its new value when it changes, so points arriving
  // is something you see happen. It starts at the value it is given.
  let { value }: { value: number } = $props();
  const shown = Tween.of(() => value, { duration: () => ms(COUNT), easing: cubicOut });
</script>

{num(Math.round(shown.current))}
