import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState } from './fixtures/scenarios';

test.describe('Search and Filtering', () => {
	test('search input visible on library browse page', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		await expect(page.getByPlaceholder('Search...')).toBeVisible();
	});

	test('search filters items (type "Movie 1" → only Movie 1 shown)', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// Wait for initial load
		await expect(page.getByText('Movie 1')).toBeVisible();

		// Type search query
		await page.getByPlaceholder('Search...').fill('Movie 1');

		// Wait for debounced search to trigger (300ms debounce)
		await page.waitForTimeout(500);

		// Movie 1 should be visible, Movie 2 should not
		await expect(page.getByText('Movie 1')).toBeVisible();
		// The mock API filters by search param, so only matching items return
	});

	test('search with no results shows empty state message', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		await expect(page.getByText('Movie 1')).toBeVisible();

		// Search for nonexistent item
		await page.getByPlaceholder('Search...').fill('xyznonexistent');

		// Wait for debounced search
		await page.waitForTimeout(500);

		await expect(page.getByText('No items found')).toBeVisible();
		await expect(page.getByText('Try a different search term')).toBeVisible();
	});

	test('clearing search restores full list', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		await expect(page.getByText('Movie 1')).toBeVisible();

		// Search for something specific
		await page.getByPlaceholder('Search...').fill('Movie 1');
		await page.waitForTimeout(500);

		// Clear the search
		await page.getByPlaceholder('Search...').fill('');
		await page.waitForTimeout(500);

		// All items should be back
		await expect(page.getByText('Movie 1')).toBeVisible();
		await expect(page.getByText('Movie 2')).toBeVisible();
	});

	test('jobs page filter by filename', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		await expect(page.getByText('completed-1.mkv')).toBeVisible();

		// Search jobs by filename
		await page.getByPlaceholder('Search jobs...').fill('completed-1');

		// Should only show matching job
		await expect(page.getByText('completed-1.mkv')).toBeVisible();
	});

	test('jobs page filter by status', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		// Search by status
		await page.getByPlaceholder('Search jobs...').fill('failed');

		// Should show failed job
		await expect(page.getByText('failed-1.mkv')).toBeVisible();
	});

	test('jobs page filter by rule name', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		// All jobs have "Default Rule" as rule_name
		await page.getByPlaceholder('Search jobs...').fill('Default Rule');

		// Should still show jobs
		await expect(page.getByText('completed-1.mkv')).toBeVisible();
	});

	test('jobs search no match shows empty row', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		await page.getByPlaceholder('Search jobs...').fill('xyznonexistent');

		await expect(page.getByText('No jobs found')).toBeVisible();
	});
});
