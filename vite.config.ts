import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  envPrefix: ['VITE_'],
  build: { target: ['es2022', 'safari15'] },
  test: { include: ['src/**/*.test.ts'] },
});
