import { defineConfig } from 'vite';

export default defineConfig({
  // Relative asset paths, so dist/ works from any sub-path (e.g. GitHub Pages).
  base: './',
  // The wasm package is built into web/pkg, outside the app root.
  server: { fs: { allow: ['..'] } },
  worker: { format: 'es' },
  build: { target: 'es2022' },
});
