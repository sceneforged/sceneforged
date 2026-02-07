import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	define: {
		__GIT_SHA__: JSON.stringify('test')
	},
	plugins: [tailwindcss(), svelte({ hot: false })],
	resolve: {
		conditions: ['browser'],
		alias: {
			$lib: new URL('./src/lib', import.meta.url).pathname,
			'$app/environment': new URL('./src/test-mocks/app-environment.ts', import.meta.url)
				.pathname,
			'$app/navigation': new URL('./src/test-mocks/app-navigation.ts', import.meta.url)
				.pathname,
			'$app/state': new URL('./src/test-mocks/app-state.ts', import.meta.url).pathname
		}
	},
	test: {
		environment: 'jsdom',
		setupFiles: ['./src/test-setup.ts'],
		include: ['src/**/*.test.ts'],
		globals: true
	}
});
