import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Port 1420 with strictPort is what Tauri's devUrl expects by default.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // The dev server watches the project for changes, and on Windows a
    // watched folder cannot be renamed or deleted. Build output and the
    // portable app are not source, so they are left alone.
    watch: { ignored: ['**/src-tauri/**', '**/portable/**'] },
  },
});
