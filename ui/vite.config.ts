import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import { execSync } from 'node:child_process';

let gitSha = process.env.PUBLIC_COMMIT_SHA || 'unknown';
if (gitSha === 'unknown' || gitSha === 'dev') {
	try {
		gitSha = execSync('git rev-parse --short HEAD').toString().trim();
	} catch {
		// git not available (e.g. Docker build without COMMIT_SHA)
	}
}
// Truncate to short hash (7 chars) if a full SHA was provided
if (gitSha.length > 7 && gitSha !== 'unknown') {
	gitSha = gitSha.substring(0, 7);
}

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	define: {
		__GIT_SHA__: JSON.stringify(gitSha)
	},
	server: {
		proxy: {
			'/api': {
				target: 'http://localhost:8080',
				configure: (proxy) => {
					proxy.on('proxyRes', (proxyRes) => {
						if (proxyRes.headers['content-type']?.includes('text/event-stream')) {
							proxyRes.headers['cache-control'] = 'no-cache';
							proxyRes.headers['x-accel-buffering'] = 'no';
						}
					});
				}
			},
			'/webhook': 'http://localhost:8080',
			'/metrics': 'http://localhost:8080',
			'/health': 'http://localhost:8080'
		}
	}
});
