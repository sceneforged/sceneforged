import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState, emptyState } from './fixtures/scenarios';

test.describe('Admin Pages', () => {
	test.describe('Dashboard', () => {
		test('dashboard stats render', async ({ page }) => {
			const api = new MockApi(page);
			const scenario = populatedState();
			await api.setup(scenario);

			await page.goto('/admin');

			// Should display key stats
			await expect(page.getByText(String(scenario.dashboard.total_libraries))).toBeVisible();
			await expect(page.getByText(String(scenario.dashboard.total_items))).toBeVisible();
		});
	});

	test.describe('Libraries Admin', () => {
		test('library list displays libraries', async ({ page }) => {
			const api = new MockApi(page);
			const scenario = populatedState();
			await api.setup(scenario);

			await page.goto('/admin/libraries');

			// Scope to main content and use heading role to avoid matching sidebar/badge/path duplicates
			const main = page.locator('main');
			await expect(main.getByRole('heading', { name: 'Movies' })).toBeVisible();
			await expect(main.getByRole('heading', { name: 'TV Shows' })).toBeVisible();
		});

		test('shows empty state when no libraries', async ({ page }) => {
			const api = new MockApi(page);
			await api.setup(emptyState());

			await page.goto('/admin/libraries');

			const main = page.locator('main');
			await expect(main.getByText('No libraries')).toBeVisible();
		});
	});

	test.describe('Jobs', () => {
		test('job queue and history display', async ({ page }) => {
			const api = new MockApi(page);
			const scenario = populatedState();
			await api.setup(scenario);

			await page.goto('/admin/jobs');

			// Should show "Job History" section
			await expect(page.getByText('Job History')).toBeVisible();

			// Should show completed jobs
			await expect(page.getByText('completed-1.mkv')).toBeVisible();
		});

		test('rules section at bottom of jobs page', async ({ page }) => {
			const api = new MockApi(page);
			const scenario = populatedState();
			await api.setup(scenario);

			await page.goto('/admin/jobs');

			// Expand rules section
			await page.getByRole('button', { name: /Processing Rules/ }).click();

			// Should show rules
			await expect(page.getByText('Transcode 4K')).toBeVisible();
			await expect(page.getByText('Extract Subtitles')).toBeVisible();
			await expect(page.getByText('Legacy Format')).toBeVisible();
		});

		test('rules section shows empty state when no rules', async ({ page }) => {
			const api = new MockApi(page);
			await api.setup(emptyState());

			await page.goto('/admin/jobs');

			await page.locator('[data-slot="collapsible-trigger"]').click();
			await expect(page.getByText('No rules configured')).toBeVisible();
		});
	});
});
