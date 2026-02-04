import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState, authDisabledState } from './fixtures/scenarios';

test.describe('Authentication Flow', () => {
	test('login page renders form elements', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Override auth to indicate not authenticated
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

		// CardTitle renders as div, not heading
		await expect(page.getByText('Sign In').first()).toBeVisible();
		await expect(page.locator('input#username')).toBeVisible();
		await expect(page.locator('input#password')).toBeVisible();
		await expect(page.getByRole('button', { name: 'Sign In' })).toBeVisible();
	});

	test('login with valid credentials redirects to /', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Start unauthenticated
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

		// Login succeeds
		await api.overrideRoute('**/api/auth/login', (route) => {
			if (route.request().method() === 'POST') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({ success: true })
				});
			}
			return route.fallback();
		});

		await page.goto('/login');
		await page.locator('input#username').fill('admin');
		await page.locator('input#password').fill('password');
		await page.getByRole('button', { name: 'Sign In' }).click();

		await expect(page).toHaveURL('/');
	});

	test('login with invalid credentials shows error message', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

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

		await api.overrideRoute('**/api/auth/login', (route) => {
			if (route.request().method() === 'POST') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({ success: false })
				});
			}
			return route.fallback();
		});

		await page.goto('/login');
		await page.locator('input#username').fill('admin');
		await page.locator('input#password').fill('wrong');
		await page.getByRole('button', { name: 'Sign In' }).click();

		await expect(page.getByText('Login failed')).toBeVisible();
	});

	test('empty fields show client-side validation error', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

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

		// Click sign in without filling fields
		await page.getByRole('button', { name: 'Sign In' }).click();

		await expect(page.getByText('Please enter username and password')).toBeVisible();
	});

	test('auth disabled auto-redirects away from login', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = authDisabledState();
		await api.setup(scenario);

		// Auth is disabled
		await api.overrideRoute('**/api/auth/status', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					authenticated: false,
					auth_enabled: false
				})
			})
		);

		await page.goto('/login');

		// Should redirect to / since auth is disabled
		await expect(page).toHaveURL('/');
	});

	test('already authenticated redirects away from login', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Already authenticated (default mock)
		await page.goto('/login');

		// Should redirect to /
		await expect(page).toHaveURL('/');
	});

	test('logout button triggers logout', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		let logoutCalled = false;
		// Mock logout
		await api.overrideRoute('**/api/auth/logout', (route) => {
			if (route.request().method() === 'POST') {
				logoutCalled = true;
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: '{}'
				});
			}
			return route.fallback();
		});

		await page.goto('/');
		await expect(page.getByLabel('Logout')).toBeVisible();

		await page.getByLabel('Logout').click();

		// Wait for navigation after logout
		await page.waitForTimeout(1000);

		// The logout API should have been called
		expect(logoutCalled).toBe(true);
	});

	test('login button disabled during submission', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

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

		// Delay login response to observe disabled state
		await api.overrideRoute('**/api/auth/login', (route) => {
			if (route.request().method() === 'POST') {
				return new Promise((resolve) => {
					setTimeout(() => {
						resolve(
							route.fulfill({
								status: 200,
								contentType: 'application/json',
								body: JSON.stringify({ success: true })
							})
						);
					}, 2000);
				});
			}
			return route.fallback();
		});

		await page.goto('/login');
		await page.locator('input#username').fill('admin');
		await page.locator('input#password').fill('password');

		const submitBtn = page.getByRole('button', { name: /Sign/i });
		await submitBtn.click();

		// Button should be disabled during submission
		await expect(submitBtn).toBeDisabled();
		// Should show "Signing in..." text
		await expect(page.getByText('Signing in...')).toBeVisible();
	});
});
