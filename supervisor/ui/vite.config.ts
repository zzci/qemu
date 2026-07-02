import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Served by vmd on the same origin as the API/WS, so relative URLs work in production.
// For local dev (`npm run dev`), proxy the vmd endpoints to a running vmd on :8006.
export default defineConfig({
  plugins: [react()],
  base: './',
  // noVNC uses top-level await (WebCodecs feature-detect) -> needs a modern target.
  build: { outDir: 'dist', emptyOutDir: true, target: 'esnext' },
  optimizeDeps: { esbuildOptions: { target: 'esnext' } },
  server: {
    proxy: {
      '/websockify': { target: 'ws://localhost:8006', ws: true },
      '/console': { target: 'ws://localhost:8006', ws: true },
      '/power': 'http://localhost:8006',
      '/status': 'http://localhost:8006',
    },
  },
})
