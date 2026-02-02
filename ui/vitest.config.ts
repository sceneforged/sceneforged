import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [
    svelte({
      hot: false,
      compilerOptions: {
        // Disable warnings during tests
        warningFilter: () => false,
      },
    }),
  ],
  test: {
    // Use jsdom for browser-like environment
    environment: 'jsdom',

    // Include test files
    include: ['tests/**/*.test.ts', 'tests/**/*.test.svelte.ts'],

    // Exclude e2e tests (run with Playwright)
    exclude: ['tests/e2e/**/*', 'node_modules/**/*'],

    // Setup files
    setupFiles: ['./tests/setup.ts'],

    // Enable globals like describe, it, expect
    globals: true,

    // Coverage configuration
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      include: ['src/**/*.ts', 'src/**/*.svelte'],
      exclude: ['src/**/*.d.ts', 'src/app.html'],
    },

    // Resolve aliases from svelte-kit
    alias: {
      $lib: '/src/lib',
      $app: '/.svelte-kit/runtime/app',
    },
  },
});
