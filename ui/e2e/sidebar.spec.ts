import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { emptyState, populatedState } from './fixtures/scenarios';

test.describe('Sidebar Navigation', () => {
	test('shows "No Libraries" when library list is empty', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(emptyState());
		await page.goto('/');

		await expect(page.getByText('No Libraries')).toBeVisible();
	});

	test('lists libraries dynamically', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		await expect(page.getByRole('link', { name: 'Movies' })).toBeVisible();
		await expect(page.getByRole('link', { name: 'TV Shows' })).toBeVisible();
	});

	test('highlights active route', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/admin');

		const dashboardLink = page.locator('[data-sidebar]').getByRole('link', { name: 'Dashboard' });
		await expect(dashboardLink).toHaveAttribute('data-active', 'true');
	});

	test('navigates to library browse on click', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);
		await page.goto('/');

		const movieLib = scenario.libraries[0];
		await page.locator('[data-sidebar]').getByRole('link', { name: 'Movies' }).click();

		await expect(page).toHaveURL(new RegExp(`/browse/${movieLib.id}`));
	});

	test('admin section has correct links', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		const sidebar = page.locator('[data-slot="sidebar-inner"], [data-mobile="true"]').first();
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
		await expect(sidebar.getByRole('link', { name: 'Settings' })).toHaveAttribute(
			'href',
			'/settings'
		);
	});

	test('mobile sidebar toggle', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());

		await page.setViewportSize({ width: 375, height: 667 });
		await page.goto('/');

		// Sidebar content should not be visible initially on mobile
		await expect(page.locator('[data-mobile="true"]')).not.toBeVisible();

		// Open sidebar via trigger button
		await page.getByRole('button', { name: 'Toggle Sidebar' }).click();

		// Sidebar sheet should now be visible
		await expect(page.locator('[data-mobile="true"]')).toBeVisible();
	});

	test('theme toggle switches theme', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		const toggle = page.getByLabel('Toggle theme');
		await toggle.click();

		// After toggling, the html element should have the dark class (or not)
		const hasDark = await page.locator('html').evaluate((el) => el.classList.contains('dark'));
		// Toggle again
		await toggle.click();
		const hasDarkAfter = await page.locator('html').evaluate((el) =>
			el.classList.contains('dark')
		);

		// They should be different
		expect(hasDark).not.toBe(hasDarkAfter);
	});

	test('logout button is visible when authenticated', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		await expect(page.getByLabel('Logout')).toBeVisible();
	});
});
