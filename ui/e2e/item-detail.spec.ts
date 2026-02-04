import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState, noWebCompatibleState } from './fixtures/scenarios';
import { createItem, createMediaFile } from './fixtures/factories';

test.describe('Item Detail Pages', () => {
	test('item detail renders metadata (name, year, runtime, rating, overview)', async ({
		page
	}) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0]; // Movie 1
		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		// Name
		await expect(page.getByRole('heading', { name: item.name })).toBeVisible();

		// Year
		await expect(page.getByText(String(item.year))).toBeVisible();

		// Runtime (120 minutes = 2h 0m)
		await expect(page.getByText('2h 0m')).toBeVisible();

		// Rating
		await expect(page.getByText('7.5')).toBeVisible();

		// Overview
		await expect(page.getByText('A test movie for e2e testing.')).toBeVisible();
	});

	test('play button visible for web-playable items (profile B / universal)', async ({
		page
	}) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Movie 1 has profile B, role universal
		const item = scenario.items[0];
		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		await expect(page.getByRole('button', { name: 'Play' })).toBeVisible();
	});

	test('"Needs Conversion" state for source-only items', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = noWebCompatibleState();
		await api.setup(scenario);

		const item = scenario.items[0];
		const lib = scenario.libraries[0];
		await page.goto(`/browse/${lib.id}/${item.id}`);

		await expect(page.getByRole('button', { name: 'Needs Conversion' })).toBeVisible();
		await expect(
			page.getByText('This item is not yet available for playback.')
		).toBeVisible();
	});

	test('media files section shows codec/resolution/size', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0];
		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		// Media Files heading
		await expect(page.getByRole('heading', { name: 'Media Files' })).toBeVisible();

		// Codec (HEVC uppercase)
		await expect(page.getByText('HEVC')).toBeVisible();

		// Resolution
		await expect(page.getByText('1920x1080')).toBeVisible();

		// File size (1.5GB = 1.4 GB)
		await expect(page.getByText(/1\.\d\s*GB/)).toBeVisible();
	});

	test('episode info (season/episode numbers) for TV items', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Find the first episode item
		const episode = scenario.items.find((i) => i.item_kind === 'episode')!;
		const tvLib = scenario.libraries[1];
		await page.goto(`/browse/${tvLib.id}/${episode.id}`);

		await expect(page.getByText('Episode Info')).toBeVisible();
		await expect(page.getByText('Season 1')).toBeVisible();
		// "Episode 1" appears both in the heading (item name) and Episode Info section
		await expect(page.getByText('Episode 1').first()).toBeVisible();
	});

	test('back to Library button navigates correctly', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0];
		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		await page.getByRole('button', { name: 'Back to Library' }).click();
		await expect(page).toHaveURL(new RegExp(`/browse/${movieLib.id}`));
	});

	test('item with missing optional fields does not crash', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0];
		// Override item to have no optional fields
		await api.overrideRoute('**/api/items/*', (route) => {
			const url = route.request().url();
			if (url.includes(item.id)) {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({
						id: item.id,
						library_id: item.library_id,
						item_kind: 'movie',
						name: 'Minimal Movie',
						year: null,
						overview: null,
						runtime_minutes: null,
						community_rating: null,
						images: [],
						media_files: [],
						created_at: new Date().toISOString(),
						updated_at: new Date().toISOString()
					})
				});
			}
			return route.fallback();
		});

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		// Should render without crashing
		await expect(page.getByRole('heading', { name: 'Minimal Movie' })).toBeVisible();
	});

	test('play button navigates to /play/[itemId]', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const item = scenario.items[0]; // Has profile B, web-playable
		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}/${item.id}`);

		await page.getByRole('button', { name: 'Play' }).click();
		await expect(page).toHaveURL(new RegExp(`/play/${item.id}`));
	});
});
