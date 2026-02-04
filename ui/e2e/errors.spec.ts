import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState } from './fixtures/scenarios';

test.describe('Error & Failure Handling', () => {
	test('API 500 on libraries shows error state', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/libraries', (route) => {
			if (route.request().method() === 'GET') {
				return route.fulfill({
					status: 500,
					contentType: 'application/json',
					body: JSON.stringify({ message: 'Internal server error' })
				});
			}
			return route.fulfill({ status: 200, body: '{}' });
		});

		await page.goto('/admin/libraries');

		// API client retries 3 times on 500 (total ~3.5s delay)
		await expect(page.getByText('Internal server error')).toBeVisible({ timeout: 15000 });
	});

	test('API 500 on items shows error + retry button', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/items?**', (route) =>
			route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ message: 'Server error' })
			})
		);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// Should show error after retries exhaust
		await expect(page.getByText('Server error')).toBeVisible({ timeout: 15000 });

		// Try Again button should be present
		await expect(page.getByRole('button', { name: 'Try Again' })).toBeVisible();
	});

	test('API 404 for nonexistent item', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Override individual item route to return 404 with a body
		await api.overrideRoute('**/api/items/*', (route) => {
			const url = route.request().url();
			if (url.includes('nonexistent-item-id')) {
				return route.fulfill({
					status: 404,
					contentType: 'application/json',
					body: JSON.stringify({ message: 'Item not found' })
				});
			}
			return route.fallback();
		});

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/nonexistent-item-id`);

		// Item detail page shows the error message or fallback
		await expect(page.getByText('Item not found')).toBeVisible({ timeout: 10000 });
	});

	test('API 404 for nonexistent library', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Override individual library route to return 404 with a body
		await api.overrideRoute('**/api/libraries/*', (route) => {
			const url = route.request().url();
			if (url.includes('nonexistent-library-id')) {
				return route.fulfill({
					status: 404,
					contentType: 'application/json',
					body: JSON.stringify({ message: 'Library not found' })
				});
			}
			return route.fallback();
		});

		await page.goto('/browse/nonexistent-library-id');

		// Browse page shows error text
		await expect(page.getByText('Library not found')).toBeVisible({ timeout: 10000 });
	});

	test('API 500 on dashboard', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/admin/dashboard', (route) =>
			route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ message: 'Dashboard error' })
			})
		);

		await page.goto('/admin');

		await expect(page.getByText('Dashboard error')).toBeVisible({ timeout: 15000 });
	});

	test('API 500 on jobs page', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/jobs?**', (route) =>
			route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ message: 'Jobs error' })
			})
		);
		await api.overrideRoute('**/api/jobs', (route) =>
			route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ message: 'Jobs error' })
			})
		);

		await page.goto('/admin/jobs');

		await expect(page.getByText('Jobs error')).toBeVisible({ timeout: 15000 });
	});

	test('API 500 on rules section', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/config/rules', (route) =>
			route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ message: 'Rules error' })
			})
		);

		await page.goto('/admin/jobs');

		// Expand rules section
		await page.getByRole('button', { name: /Processing Rules/ }).click();

		await expect(page.getByText('Rules error')).toBeVisible({ timeout: 15000 });
	});

	test('API 500 on settings/tools', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/admin/tools', (route) =>
			route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ message: 'Tools error' })
			})
		);

		await page.goto('/settings');

		await expect(page.getByText('Tools error')).toBeVisible({ timeout: 15000 });
	});

	test('client retries on 500 (succeeds on 3rd attempt)', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		let callCount = 0;
		await api.overrideRoute('**/api/admin/dashboard', (route) => {
			callCount++;
			if (callCount < 3) {
				return route.fulfill({
					status: 500,
					contentType: 'application/json',
					body: JSON.stringify({ message: 'Temporary error' })
				});
			}
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(scenario.dashboard)
			});
		});

		await page.goto('/admin');

		// Should eventually show dashboard data after retry
		await expect(
			page.getByText(String(scenario.dashboard.jobs.total)).first()
		).toBeVisible({ timeout: 15000 });
	});

	test('API returns array instead of {items, total}', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Return raw array
		const rawItems = scenario.items.filter(
			(i) => i.library_id === scenario.libraries[0].id
		);
		await api.overrideRoute('**/api/items?**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(rawItems)
			})
		);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// Should handle gracefully - getItems normalizes arrays
		await expect(page.getByText('Movie 1')).toBeVisible();
	});
});
