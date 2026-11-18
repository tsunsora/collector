import { mount } from 'svelte';
import './styles/tokens.css';
import './styles/base.css';
import App from './App.svelte';
import { app } from './lib/app.svelte';

if (import.meta.env.DEV) {
  import('./dev/audit');
  import('./lib/mockBackend').then((m) => Object.assign(window, { __app: app, __mock: m.mock }));
  import('./lib/motion.svelte').then((m) => Object.assign(window, { __holdStill: m.holdStill }));
}

void app.init();

export default mount(App, { target: document.getElementById('app')! });
