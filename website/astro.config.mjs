import { defineConfig } from 'astro/config';

// https://docs.astro.build/en/reference/configuration-reference/
export default defineConfig({
  // Project pages site: https://enekocm7.github.io/cube-app/
  site: 'https://enekocm7.github.io',
  base: '/cube-app/',
  output: 'static',
  redirects: {
    '/docs': '/cube-app/docs/install/',
  },
  build: {
    format: 'directory',
    // One less request before first paint: the whole stylesheet is under 5 KB gzipped.
    inlineStylesheets: 'always',
  },
  // Pages are about 10 KB, so fetch every visible link before it is clicked.
  prefetch: {
    prefetchAll: true,
    defaultStrategy: 'viewport',
  },
  // Cube Mono lives in ../fonts, shared with the app.
  vite: {
    server: {
      fs: {
        allow: ['..'],
      },
    },
  },
  devToolbar: {
    enabled: false,
  },
});
