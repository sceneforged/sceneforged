import type { Page } from '@playwright/test';
import type { Scenario } from '../fixtures/scenarios';

export class MockApi {
	private page: Page;
	private scenario: Scenario | null = null;

	constructor(page: Page) {
		this.page = page;
	}

	async setup(scenario: Scenario): Promise<void> {
		this.scenario = scenario;

		// Auth status — always authenticated
		await this.page.route('**/api/auth/status', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					authenticated: true,
					username: 'admin',
					auth_enabled: true
				})
			})
		);

		// Libraries
		await this.page.route('**/api/libraries', (route) => {
			if (route.request().method() === 'GET') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify(scenario.libraries)
				});
			}
			return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
		});

		// Individual library
		await this.page.route('**/api/libraries/*', (route) => {
			const url = route.request().url();
			const id = url.split('/api/libraries/')[1]?.split('/')[0];
			const lib = scenario.libraries.find((l) => l.id === id);
			if (route.request().method() === 'DELETE') {
				return route.fulfill({ status: 204 });
			}
			if (lib) {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify(lib)
				});
			}
			return route.fulfill({ status: 404 });
		});

		// Items
		await this.page.route('**/api/items?**', (route) => {
			const url = new URL(route.request().url());
			const libraryId = url.searchParams.get('library_id');
			const search = url.searchParams.get('search')?.toLowerCase();
			let items = scenario.items;

			if (libraryId) {
				items = items.filter((i) => i.library_id === libraryId);
			}
			if (search) {
				items = items.filter((i) => i.name.toLowerCase().includes(search));
			}

			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items, total: items.length })
			});
		});

		// Items without query params
		await this.page.route('**/api/items', (route) => {
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items: scenario.items, total: scenario.items.length })
			});
		});

		// Individual item
		await this.page.route('**/api/items/*', (route) => {
			const url = route.request().url();
			const id = url.split('/api/items/')[1]?.split('?')[0];
			const item = scenario.items.find((i) => i.id === id);
			if (item) {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify(item)
				});
			}
			return route.fulfill({ status: 404 });
		});

		// Jobs
		await this.page.route('**/api/jobs?**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(scenario.jobs)
			})
		);

		await this.page.route('**/api/jobs', (route) => {
			if (route.request().method() === 'GET') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify(scenario.jobs)
				});
			}
			return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
		});

		// Individual job
		await this.page.route('**/api/jobs/*', (route) => {
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					id: 'job-1',
					file_path: '/test/file.mkv',
					file_name: 'file.mkv',
					status: 'queued',
					progress: 0,
					retry_count: 0,
					max_retries: 3,
					priority: 0,
					created_at: new Date().toISOString()
				})
			});
		});

		// Dashboard
		await this.page.route('**/api/admin/dashboard', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(scenario.dashboard)
			})
		);

		// Rules
		await this.page.route('**/api/config/rules', (route) => {
			if (route.request().method() === 'GET') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify(scenario.rules)
				});
			}
			// PUT — return the sent body
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: route.request().postData() ?? '[]'
			});
		});

		// Config - Arrs
		await this.page.route('**/api/config/arrs/**', (route) => {
			const method = route.request().method();
			if (method === 'PUT') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: route.request().postData() ?? '{}'
				});
			}
			if (method === 'DELETE') {
				return route.fulfill({ status: 204 });
			}
			if (method === 'POST') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({ success: true, message: 'Connection successful' })
				});
			}
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: '[]'
			});
		});

		await this.page.route('**/api/config/arrs', (route) => {
			if (route.request().method() === 'POST') {
				return route.fulfill({
					status: 201,
					contentType: 'application/json',
					body: route.request().postData() ?? '{}'
				});
			}
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([])
			});
		});

		// Config - Jellyfins
		await this.page.route('**/api/config/jellyfins/**', (route) => {
			const method = route.request().method();
			if (method === 'PUT') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: route.request().postData() ?? '{}'
				});
			}
			if (method === 'DELETE') {
				return route.fulfill({ status: 204 });
			}
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: '[]'
			});
		});

		await this.page.route('**/api/config/jellyfins', (route) => {
			if (route.request().method() === 'POST') {
				return route.fulfill({
					status: 201,
					contentType: 'application/json',
					body: route.request().postData() ?? '{}'
				});
			}
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([])
			});
		});

		// Config - Conversion
		await this.page.route('**/api/config/conversion', (route) => {
			if (route.request().method() === 'PUT') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: route.request().postData() ?? '{}'
				});
			}
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					auto_convert_on_scan: false,
					auto_convert_dv_p7_to_p8: false,
					video_crf: 15,
					video_preset: 'slow',
					audio_bitrate: '256k',
					adaptive_crf: true
				})
			});
		});

		// Config - Reload
		await this.page.route('**/api/config/reload', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ status: 'reloaded' })
			})
		);

		// Config - Browse
		await this.page.route('**/api/config/browse**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ entries: [] })
			})
		);

		// Tools
		await this.page.route('**/api/admin/tools', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(scenario.tools)
			})
		);

		// Conversions
		await this.page.route('**/api/conversions**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([])
			})
		);

		// Playback (continue watching, user-data, progress)
		await this.page.route('**/api/playback/**', (route) => {
			if (route.request().method() === 'POST') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({
						item_id: '',
						position_secs: 0,
						completed: false,
						play_count: 0,
						last_played_at: ''
					})
				});
			}
			// GET /playback/:id/user-data
			const url = route.request().url();
			if (url.includes('/user-data')) {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({ playback: null, is_favorite: false })
				});
			}
			return route.fulfill({ status: 404 });
		});

		// Continue watching — now returns enriched entries
		await this.page.route('**/api/playback/continue**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([])
			})
		);

		// Favorites — now returns enriched entries
		await this.page.route('**/api/favorites**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([])
			})
		);

		// SSE events — return a single heartbeat then close
		await this.page.route('**/api/events', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: 'data: {"id":"hb-1","timestamp":"2024-01-01T00:00:00Z","category":"admin","payload":{"type":"heartbeat"}}\n\n'
			})
		);

		// Images — return a placeholder
		await this.page.route('**/api/images/**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'image/jpeg',
				body: Buffer.from([])
			})
		);
	}

	async overrideRoute(
		pattern: string,
		handler: (route: import('@playwright/test').Route) => Promise<void> | void
	): Promise<void> {
		await this.page.route(pattern, handler);
	}
}
