import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import { execSync } from 'node:child_process';

const gitSha = execSync('git rev-parse --short HEAD').toString().trim();

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	define: {
		__GIT_SHA__: JSON.stringify(gitSha)
	},
	server: {
		proxy: {
			'/api': 'http://localhost:8080',
			'/webhook': 'http://localhost:8080',
			'/metrics': 'http://localhost:8080',
			'/health': 'http://localhost:8080'
		}
	}
});
