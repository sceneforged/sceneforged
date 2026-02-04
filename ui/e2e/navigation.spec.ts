import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState } from './fixtures/scenarios';

test.describe('Full Navigation Flows', () => {
	test('Home -> Library -> Item Detail', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Start at home
		await page.goto('/');
		await expect(page).toHaveURL('/');

		// Click a library in sidebar
		const movieLib = scenario.libraries[0];
		await page.locator('[data-sidebar="sidebar"]').getByRole('link', { name: 'Movies' }).click();
		await expect(page).toHaveURL(new RegExp(`/browse/${movieLib.id}`));

		// Click an item
		await page.getByText('Movie 1').first().click();

		// Should navigate to item detail or play page
		await expect(page).toHaveURL(/\/(browse|play)\//);
	});

	test('Admin Dashboard -> Libraries', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin');
		await expect(page).toHaveURL('/admin');

		// Navigate to libraries via sidebar
		await page.locator('[data-sidebar="sidebar"]').getByRole('link', { name: 'Libraries' }).click();
		await expect(page).toHaveURL('/admin/libraries');
	});

	test('Settings page loads', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());

		await page.goto('/');
		await page.locator('[data-sidebar="sidebar"]').getByRole('link', { name: 'Settings' }).click();
		await expect(page).toHaveURL('/settings');
	});

	test('navigate between admin pages', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());

		await page.goto('/admin');

		// Dashboard -> Jobs
		await page.locator('[data-sidebar="sidebar"]').getByRole('link', { name: 'Jobs' }).click();
		await expect(page).toHaveURL('/admin/jobs');

		// Jobs -> Libraries
		await page.locator('[data-sidebar="sidebar"]').getByRole('link', { name: 'Libraries' }).click();
		await expect(page).toHaveURL('/admin/libraries');

		// Libraries -> Dashboard
		await page.locator('[data-sidebar="sidebar"]').getByRole('link', { name: 'Dashboard' }).click();
		await expect(page).toHaveURL('/admin');
	});

	test('Home link returns to root', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());

		await page.goto('/admin/jobs');

		await page.locator('[data-sidebar="sidebar"]').getByRole('link', { name: 'Home' }).click();
		await expect(page).toHaveURL('/');
	});
});
