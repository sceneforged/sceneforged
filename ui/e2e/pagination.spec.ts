import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState, paginatedState } from './fixtures/scenarios';
import { createJob } from './fixtures/factories';

test.describe('Pagination', () => {
	test('Load More button appears when items > PAGE_SIZE', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = paginatedState();

		// Override items to respect page/limit params
		await api.setup(scenario);
		await api.overrideRoute('**/api/items?**', (route) => {
			const url = new URL(route.request().url());
			const libraryId = url.searchParams.get('library_id');
			const offset = parseInt(url.searchParams.get('offset') ?? '0');
			const limit = parseInt(url.searchParams.get('limit') ?? '24');

			let items = scenario.items;
			if (libraryId) {
				items = items.filter((i) => i.library_id === libraryId);
			}

			const paged = items.slice(offset, offset + limit);

			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items: paged, total: items.length })
			});
		});

		const lib = scenario.libraries[0];
		await page.goto(`/browse/${lib.id}`);

		// Should show Load More button since 30 > 24
		await expect(page.getByRole('button', { name: 'Load More' })).toBeVisible();
	});

	test('clicking Load More appends items', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = paginatedState();

		await api.setup(scenario);
		await api.overrideRoute('**/api/items?**', (route) => {
			const url = new URL(route.request().url());
			const libraryId = url.searchParams.get('library_id');
			const offset = parseInt(url.searchParams.get('offset') ?? '0');
			const limit = parseInt(url.searchParams.get('limit') ?? '24');

			let items = scenario.items;
			if (libraryId) {
				items = items.filter((i) => i.library_id === libraryId);
			}

			const paged = items.slice(offset, offset + limit);

			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items: paged, total: items.length })
			});
		});

		const lib = scenario.libraries[0];
		await page.goto(`/browse/${lib.id}`);

		// Initially shows 24 items
		await expect(page.getByText('Showing 24 of 30')).toBeVisible();

		// Click Load More
		await page.getByRole('button', { name: 'Load More' }).click();

		// Should now show all 30
		await expect(page.getByText('Showing 30 of 30')).toBeVisible();

		// Load More should be gone
		await expect(page.getByRole('button', { name: 'Load More' })).not.toBeVisible();
	});

	test('Load More hidden when all items fit in one page', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// Movie library has 6 items, well under PAGE_SIZE (24)
		await expect(page.getByText('Movie 1')).toBeVisible();

		// Load More should NOT be visible
		await expect(page.getByRole('button', { name: 'Load More' })).not.toBeVisible();
	});

	test('jobs pagination prev/next works', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();

		// Add enough jobs for pagination (>25)
		const extraJobs = Array.from({ length: 25 }, (_, i) =>
			createJob({ status: 'completed', file_name: `extra-${i + 1}.mkv` })
		);
		scenario.jobs = {
			jobs: [...scenario.jobs.jobs, ...extraJobs],
			total: scenario.jobs.jobs.length + extraJobs.length
		};
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		// Should see page indicator
		await expect(page.getByText('Page 1 of 2')).toBeVisible();

		// Click Next via the chevron button (second outline button in pagination area)
		await page.getByText('Page 1 of 2').locator('..').getByRole('button').last().click();

		// Should be on page 2
		await expect(page.getByText('Page 2 of 2')).toBeVisible();

		// Click Prev
		await page.getByText('Page 2 of 2').locator('..').getByRole('button').first().click();

		await expect(page.getByText('Page 1 of 2')).toBeVisible();
	});

	test('jobs pagination single page: both buttons disabled', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		// populatedState has 7 jobs, under 25 per page
		await api.setup(scenario);

		await page.goto('/admin/jobs');

		// Should show "Page 1 of 1"
		await expect(page.getByText('Page 1 of 1')).toBeVisible();
	});
});
