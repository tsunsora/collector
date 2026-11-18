<script lang="ts">
  import { app } from '../lib/app.svelte';
  import Notice from './Notice.svelte';

  // Twitch's private API has moved on and this version no longer speaks it.
  // Unlike "Twitch didn't respond", trying again will not help: the way
  // forward is a newer version, so that is the action when updates are set up.
  const canUpdate = $derived(app.update.state !== 'off');

  function act() {
    if (!canUpdate) return void app.retry();
    void app.checkUpdate();
    app.openMain('settings');
  }
</script>

<Notice
  icon="alert"
  title="Twitch has changed something"
  body="This version no longer understands Twitch’s answers, so collecting has stopped. Your points are safe. A newer version should fix it."
  action={canUpdate ? 'Check for updates' : 'Try again'}
  onaction={act}
/>
