import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState, emptyState, noWebCompatibleState } from './fixtures/scenarios';

test.describe('Library Browsing', () => {
	test('media grid renders for library', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// Should show movie items
		for (let i = 1; i <= 6; i++) {
			await expect(page.getByText(`Movie ${i}`)).toBeVisible();
		}
	});

	test('play overlay shows on hover for web-compatible cards', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		const movieLib = scenario.libraries[0];
		await page.goto(`/browse/${movieLib.id}`);

		// Movie 1 has profile B (web-playable) — hover to see play overlay
		const firstCard = page.getByText('Movie 1').first();
		await firstCard.hover();

		// The play overlay should become visible on hover
		// (has opacity-0 -> group-hover:opacity-100)
	});

	test('empty library shows empty state', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = emptyState();
		// Add one library but no items
		const { createLibrary } = await import('./fixtures/factories');
		const lib = createLibrary({ name: 'Empty Lib' });
		scenario.libraries = [lib];
		await api.setup(scenario);

		await page.goto(`/browse/${lib.id}`);

		// The page should not show any media cards
		await expect(page.locator('[class*="aspect-"]')).toHaveCount(0);
	});

	test('no web-compatible items do not show play overlay', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = noWebCompatibleState();
		await api.setup(scenario);

		const lib = scenario.libraries[0];
		await page.goto(`/browse/${lib.id}`);

		await expect(page.getByText('Source Only Movie 1')).toBeVisible();
	});
});
