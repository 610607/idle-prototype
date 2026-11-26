import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [
    react({
      babel: {
        babelrc: false,
        configFile: false,
      },
    }),
  ],
  css: {
    postcss: {
      plugins: [],
    },
  },
  server: {
    port: 5173,
    allowedHosts: ['.ts.net'],
  },
  test: {
    environment: 'node',
  },
});
