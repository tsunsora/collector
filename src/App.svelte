<script lang="ts">
  import Bench from './dev/Bench.svelte';
  import MainWindow from './screens/MainWindow.svelte';
  import Popover from './screens/Popover.svelte';
  import Tokens from './dev/Tokens.svelte';

  // Tauri loads each window at its own hash: index.html#popover and index.html#main.
  // With no hash, a dev build shows the preview bench and a release build the popover.
  type Route = 'popover' | 'main' | 'bench' | 'tokens';
  const fallback: Route = import.meta.env.DEV ? 'bench' : 'popover';
  const parse = (): Route => {
    const h = location.hash.slice(1);
    return h === 'popover' || h === 'main' || h === 'bench' || h === 'tokens' ? h : fallback;
  };

  let route = $state(parse());

  $effect(() => {
    const onHash = () => (route = parse());
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  });
</script>

{#if route === 'popover'}
  <Popover />
{:else if route === 'main'}
  <MainWindow />
{:else if route === 'tokens'}
  <Tokens />
{:else}
  <Bench />
{/if}
