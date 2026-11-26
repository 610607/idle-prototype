import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: './',
  plugins: [react({ babel: { babelrc: false, configFile: false } })],
  css: { postcss: { plugins: [] } },
  server: {
    port: 5174,
    allowedHosts: ['.ts.net'],
    // The shared icon set lives at the repo root (also used by the future iOS client).
    fs: { allow: ['..'] },
  },
});
