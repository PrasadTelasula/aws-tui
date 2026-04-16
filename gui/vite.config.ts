import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? true,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**']
    }
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  optimizeDeps: {
    include: [
      '@xterm/xterm',
      '@xterm/addon-fit',
      '@xterm/addon-web-links',
      'lucide-svelte',
      'clsx',
      'tailwind-merge',
      'tailwind-variants'
    ]
  }
});
