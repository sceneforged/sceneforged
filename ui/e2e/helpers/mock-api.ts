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

		// Tools
		await this.page.route('**/api/admin/tools', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(scenario.tools)
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
