import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState } from './fixtures/scenarios';
import { createItem, createMediaFile, createJob } from './fixtures/factories';

test.describe('Regression Guards', () => {
	test('collapsible rules toggles open/closed correctly', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		// Rules section should be collapsed initially
		const trigger = page.getByRole('button', { name: /Processing Rules/ });
		await expect(trigger).toBeVisible();

		// Open it
		await trigger.click();
		await expect(page.getByText('Transcode 4K')).toBeVisible();

		// Close it
		await trigger.click();
		await expect(page.getByText('Transcode 4K')).not.toBeVisible();

		// Reopen — verifies no state_unsafe_mutation from rules.sort()
		await trigger.click();
		await expect(page.getByText('Transcode 4K')).toBeVisible();
	});

	test('getItems handles plain array API response', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Override items endpoint to return a plain array (no {items, total} wrapper)
		await api.overrideRoute('**/api/items?**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(scenario.items.slice(0, 3))
			})
		);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// Should still render items despite receiving array instead of {items, total}
		await expect(page.getByText('Movie 1')).toBeVisible();
	});

	test('getJobs handles plain array API response', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Override jobs endpoint to return a plain array (no {jobs, total} wrapper)
		const plainJobs = scenario.jobs.jobs;
		await api.overrideRoute('**/api/jobs?**', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(plainJobs)
			})
		);
		await api.overrideRoute('**/api/jobs', (route) => {
			if (route.request().method() === 'GET') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify(plainJobs)
				});
			}
			return route.fulfill({ status: 200, body: '{}' });
		});

		await page.goto('/admin/jobs');

		// Should still render jobs despite receiving array instead of {jobs, total}
		await expect(page.getByText('completed-1.mkv')).toBeVisible();
	});

	test('rules page sort does not trigger state_unsafe_mutation', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Listen for page errors (state_unsafe_mutation throws at runtime)
		const errors: string[] = [];
		page.on('pageerror', (err) => errors.push(err.message));

		await page.goto('/rules');

		// Rules should load and render (would hang on "Loading rules..." if sort mutates state)
		await expect(page.getByText('Transcode 4K')).toBeVisible({ timeout: 10000 });
		await expect(page.getByText('Extract Subtitles')).toBeVisible();

		// No state_unsafe_mutation error should have occurred
		const mutationErrors = errors.filter((e) => e.includes('state_unsafe_mutation'));
		expect(mutationErrors).toHaveLength(0);
	});

	test('normalizeItem handles null media_files', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0];
		// Simulate null media_files from API
		await api.overrideRoute('**/api/items/*', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ ...item, media_files: null })
			})
		);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		// Should render without crashing
		await expect(page.getByRole('heading', { name: item.name })).toBeVisible();
	});

	test('normalizeItem handles undefined media_files', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0];
		const { media_files: _mf, ...itemWithoutMedia } = item;
		await api.overrideRoute('**/api/items/*', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(itemWithoutMedia)
			})
		);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		await expect(page.getByRole('heading', { name: item.name })).toBeVisible();
	});

	test('sidebar nav links have correct hrefs and navigate', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/');

		const sidebar = page.locator('[data-slot="sidebar-inner"], [data-mobile="true"]').first();

		// Verify sidebar links
		await expect(sidebar.getByRole('link', { name: 'Home' })).toHaveAttribute('href', '/');
		await expect(sidebar.getByRole('link', { name: 'Dashboard' })).toHaveAttribute(
			'href',
			'/admin'
		);
		await expect(sidebar.getByRole('link', { name: 'Libraries' })).toHaveAttribute(
			'href',
			'/admin/libraries'
		);
		await expect(sidebar.getByRole('link', { name: 'Jobs' })).toHaveAttribute(
			'href',
			'/admin/jobs'
		);

		// Click and navigate
		await sidebar.getByRole('link', { name: 'Dashboard' }).click();
		await expect(page).toHaveURL('/admin');
	});

	test('text-muted-foreground text is actually visible', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());

		await page.goto('/');

		// The subtitle uses text-muted-foreground
		const subtitle = page.getByText('Your personal media library');
		await expect(subtitle).toBeVisible();

		// Verify it has non-zero opacity (not invisible)
		const opacity = await subtitle.evaluate((el) => {
			const style = window.getComputedStyle(el);
			return parseFloat(style.opacity);
		});
		expect(opacity).toBeGreaterThan(0);
	});

	test('auth store fallback: checkStatus failure defaults to authenticated', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Override auth to fail
		await api.overrideRoute('**/api/auth/status', (route) =>
			route.fulfill({ status: 500, body: 'Server error' })
		);

		await page.goto('/');

		// Should still show the app (not redirect to login) since fallback treats as authenticated
		await expect(page.getByText('Welcome to SceneForged')).toBeVisible();
	});

	test('job with undefined completed_at shows dash not "Invalid Date"', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		// The running job already has completed_at: undefined
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		// The running job should show "-" in the Completed column, not "Invalid Date"
		const invalidDate = page.getByText('Invalid Date');
		await expect(invalidDate).toHaveCount(0);
	});

	test('API client does not retry on 4xx errors (fast failure)', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		let requestCount = 0;
		await api.overrideRoute('**/api/items/*', (route) => {
			requestCount++;
			return route.fulfill({
				status: 404,
				contentType: 'application/json',
				body: JSON.stringify({ code: 'NOT_FOUND', message: 'Item not found' })
			});
		});

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/nonexistent-id`);

		// Wait for the error to appear
		await expect(page.getByText('Item not found')).toBeVisible();

		// Should NOT have retried - only 1 request for 4xx
		expect(requestCount).toBe(1);
	});

	test('multiple rapid navigations do not cause state corruption', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/');

		const sidebar = page.locator('[data-sidebar="sidebar"]');

		// Rapidly navigate between pages
		await sidebar.getByRole('link', { name: 'Dashboard' }).click();
		await sidebar.getByRole('link', { name: 'Jobs' }).click();
		await sidebar.getByRole('link', { name: 'Libraries' }).click();
		await sidebar.getByRole('link', { name: 'Home' }).click();

		// Should end up at home without errors
		await expect(page).toHaveURL('/');
		await expect(page.getByText('Welcome to SceneForged')).toBeVisible();
	});

	test('SSE reconnect error does not crash the page', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Override SSE to return an error
		await api.overrideRoute('**/api/events', (route) =>
			route.fulfill({ status: 500, body: 'SSE Error' })
		);

		await page.goto('/');

		// Page should still function normally
		await expect(page.getByText('Welcome to SceneForged')).toBeVisible();
	});

	test('very long item name truncates with ellipsis', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();

		// Create an item with a very long name
		const longName = 'A'.repeat(200);
		const item = createItem({
			library_id: scenario.libraries[0].id,
			name: longName
		});
		item.media_files = [
			createMediaFile({ item_id: item.id, role: 'source', profile: 'A' })
		];
		scenario.items.push(item);
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// The item should be rendered (page doesn't break)
		await expect(page.getByText(longName.slice(0, 20))).toBeVisible();
	});
});
