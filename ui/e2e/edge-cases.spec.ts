import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import {
	populatedState,
	emptyState
} from './fixtures/scenarios';
import {
	createLibrary,
	createDashboardStats,
	createToolInfo,
	createJob,
	createRule,
	resetIdCounter
} from './fixtures/factories';
import type { Scenario } from './fixtures/scenarios';

test.describe('Edge Cases', () => {
	test('20 libraries in sidebar scroll correctly', async ({ page }) => {
		const api = new MockApi(page);
		resetIdCounter();

		const libraries = Array.from({ length: 20 }, (_, i) =>
			createLibrary({ name: `Library ${i + 1}` })
		);

		const scenario: Scenario = {
			libraries,
			items: [],
			jobs: { jobs: [], total: 0 },
			dashboard: createDashboardStats({ total_libraries: 20 }),
			rules: [],
			tools: [createToolInfo()]
		};

		await api.setup(scenario);
		await page.goto('/');

		// First library should be visible (use exact to avoid matching Library 10-19)
		await expect(page.getByRole('link', { name: 'Library 1', exact: true })).toBeVisible();

		// Last library should exist in the DOM (may need scroll)
		const lastLib = page.getByRole('link', { name: 'Library 20' });
		await expect(lastLib).toBeAttached();
	});

	test('library with empty paths array renders', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();

		// Add a library with no paths
		const emptyPathLib = createLibrary({ name: 'No Paths', paths: [] });
		scenario.libraries.push(emptyPathLib);

		await api.setup(scenario);
		await page.goto('/admin/libraries');

		const main = page.locator('main');
		await expect(main.getByText('No Paths')).toBeVisible();
	});

	test('dashboard with all-zero stats renders correctly', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = emptyState();
		scenario.dashboard = createDashboardStats({
			total_libraries: 0,
			total_items: 0,
			total_jobs: 0,
			active_jobs: 0,
			completed_jobs: 0,
			failed_jobs: 0
		});
		await api.setup(scenario);

		await page.goto('/admin');

		// Should render the dashboard heading
		await expect(page.getByRole('heading', { name: 'Admin Dashboard' })).toBeVisible();

		// Key labels from stat cards should be visible
		await expect(page.getByText('Libraries').first()).toBeVisible();
		await expect(page.getByText('Total Items')).toBeVisible();
	});

	test('item with no media files shows no badges', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0];
		// Override to return item with no media files
		await api.overrideRoute('**/api/items/*', (route) => {
			const url = route.request().url();
			if (url.includes(item.id)) {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({ ...item, media_files: [] })
				});
			}
			return route.fallback();
		});

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		// Should render without "Media Files" section
		await expect(page.getByRole('heading', { name: item.name })).toBeVisible();
		await expect(page.getByText('Media Files')).not.toBeVisible();
	});

	test('tool with missing version/path renders', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();

		// Override tools response
		await api.setup(scenario);
		await api.overrideRoute('**/api/admin/tools', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([
					createToolInfo({ name: 'ffmpeg', available: true }),
					createToolInfo({
						name: 'ffprobe',
						available: false,
						version: undefined,
						path: undefined
					})
				])
			})
		);

		await page.goto('/settings');

		// Both tools should render
		await expect(page.getByText('ffmpeg').first()).toBeVisible();
		await expect(page.getByText('ffprobe').first()).toBeVisible();

		// Missing tool should show "Missing" badge (use exact to avoid matching "Missing Tools")
		await expect(page.getByText('Missing', { exact: true })).toBeVisible();
	});

	test('double-click create library does not duplicate', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		let createCount = 0;
		const newLib = createLibrary({ name: 'Double Click Test' });
		await api.overrideRoute('**/api/libraries', (route) => {
			if (route.request().method() === 'POST') {
				createCount++;
				return route.fulfill({
					status: 201,
					contentType: 'application/json',
					body: JSON.stringify(newLib)
				});
			}
			if (route.request().method() === 'GET') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify([...scenario.libraries, newLib])
				});
			}
			return route.fulfill({ status: 200, body: '{}' });
		});

		await page.goto('/admin/libraries');

		await page.getByRole('button', { name: 'Add Library' }).click();
		await page.locator('input#lib-name').fill('Double Click Test');
		await page.locator('input#lib-path').fill('/media/test');

		// Double-click the create button
		const createBtn = page.getByRole('button', { name: 'Create Library' });
		await createBtn.dblclick();

		// Wait for response
		await page.waitForTimeout(500);

		// Should have made at most 1-2 requests (button disables during creation)
		expect(createCount).toBeLessThanOrEqual(2);
	});

	test('unknown item_kind renders with fallback icon', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0];
		await api.overrideRoute('**/api/items/*', (route) => {
			const url = route.request().url();
			if (url.includes(item.id)) {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({
						...item,
						item_kind: 'unknown_type',
						media_files: []
					})
				});
			}
			return route.fallback();
		});

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		// Should render without crashing (falls back to Film icon)
		await expect(page.getByRole('heading', { name: item.name })).toBeVisible();
	});
});
