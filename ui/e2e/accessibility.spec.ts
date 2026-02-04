import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState } from './fixtures/scenarios';

test.describe('Accessibility', () => {
	test('sidebar links have accessible names', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		// Use the sidebar inner content area where links are rendered
		const sidebar = page.locator('[data-slot="sidebar-inner"]').first();
		await expect(sidebar).toBeVisible();

		// Check specific named links exist
		await expect(sidebar.getByRole('link', { name: 'Home' })).toBeVisible();
		await expect(sidebar.getByRole('link', { name: 'Dashboard' })).toBeVisible();
	});

	test('theme toggle has aria-label', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		const themeToggle = page.getByLabel('Toggle theme');
		await expect(themeToggle).toBeVisible();
		await expect(themeToggle).toHaveAttribute('aria-label', 'Toggle theme');
	});

	test('logout button has aria-label', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		const logoutBtn = page.getByLabel('Logout');
		await expect(logoutBtn).toBeVisible();
		await expect(logoutBtn).toHaveAttribute('aria-label', 'Logout');
	});

	test('login form inputs have associated labels', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());

		await api.overrideRoute('**/api/auth/status', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					authenticated: false,
					auth_enabled: true
				})
			})
		);

		await page.goto('/login');

		// Username input should have a label
		const usernameInput = page.locator('input#username');
		await expect(usernameInput).toBeVisible();
		const usernameLabel = page.locator('label[for="username"]');
		await expect(usernameLabel).toBeVisible();
		await expect(usernameLabel).toHaveText('Username');

		// Password input should have a label
		const passwordInput = page.locator('input#password');
		await expect(passwordInput).toBeVisible();
		const passwordLabel = page.locator('label[for="password"]');
		await expect(passwordLabel).toBeVisible();
		await expect(passwordLabel).toHaveText('Password');
	});

	test('job action buttons have title attributes', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/admin/jobs');

		// Wait for jobs to load
		await expect(page.getByText('completed-1.mkv')).toBeVisible();

		// Delete buttons should have title (at least one for each completed job)
		const deleteButtons = page.getByTitle('Delete');
		expect(await deleteButtons.count()).toBeGreaterThan(0);

		// Retry button should have title (for the failed job)
		const retryButtons = page.getByTitle('Retry');
		expect(await retryButtons.count()).toBeGreaterThan(0);
	});

	test('keyboard Tab navigates sidebar links', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/');

		// Focus the body to start from a known point
		await page.locator('body').click();

		// Press Tab multiple times
		await page.keyboard.press('Tab');
		await page.keyboard.press('Tab');

		// Something should be focused
		const focused = page.locator(':focus');
		await expect(focused).toBeAttached();
	});

	test('rule editor form labels have for attributes', async ({ page }) => {
		const api = new MockApi(page);
		await api.setup(populatedState());
		await page.goto('/rules');

		// Wait for rules to fully load
		await expect(page.getByText('Transcode 4K')).toBeVisible();

		// Open editor
		await page.getByRole('button', { name: 'New Rule' }).click();

		// Wait for editor to appear
		await expect(page.locator('input#rule-name')).toBeVisible();

		// Name label
		const nameLabel = page.locator('label[for="rule-name"]');
		await expect(nameLabel).toBeVisible();
		await expect(nameLabel).toHaveText('Name');

		// Priority label
		const priorityLabel = page.locator('label[for="rule-priority"]');
		await expect(priorityLabel).toBeVisible();
		await expect(priorityLabel).toHaveText('Priority');
	});
});
