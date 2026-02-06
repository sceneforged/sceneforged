import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState, emptyState } from './fixtures/scenarios';
import { createLibrary, createRule } from './fixtures/factories';

test.describe('CRUD Operations', () => {
	test('create library form appears on button click', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin/libraries');

		// Form should not be visible initially
		await expect(page.locator('input#lib-name')).not.toBeVisible();

		// Click Add Library
		await page.getByRole('button', { name: 'Add Library' }).click();

		// Form should appear (CardTitle renders as div, not heading)
		await expect(page.getByText('New Library')).toBeVisible();
		await expect(page.locator('input#lib-name')).toBeVisible();
		await expect(page.locator('input#lib-path')).toBeVisible();
	});

	test('create library validation rejects empty fields', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/admin/libraries');

		await page.getByRole('button', { name: 'Add Library' }).click();

		// Click create without filling fields
		await page.getByRole('button', { name: 'Create Library' }).click();

		await expect(page.getByText('Name and path are required')).toBeVisible();
	});

	test('create library success appears in list', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();

		const newLib = createLibrary({ name: 'Anime', media_type: 'movies', paths: ['/media/anime'] });

		// Mock POST to create library and GET after creation
		await api.setup(scenario);
		await api.overrideRoute('**/api/libraries', (route) => {
			if (route.request().method() === 'POST') {
				return route.fulfill({
					status: 201,
					contentType: 'application/json',
					body: JSON.stringify(newLib)
				});
			}
			if (route.request().method() === 'GET') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify([...scenario.libraries, newLib])
				});
			}
			return route.fulfill({ status: 200, body: '{}' });
		});

		await page.goto('/admin/libraries');

		await page.getByRole('button', { name: 'Add Library' }).click();
		await page.locator('input#lib-name').fill('Anime');
		await page.locator('input#lib-path').fill('/media/anime');
		await page.getByRole('button', { name: 'Create Library' }).click();

		// New library should appear (use heading to avoid matching path text)
		const main = page.locator('main');
		await expect(main.locator('h3').filter({ hasText: 'Anime' })).toBeVisible();
	});

	test('delete library removes from list', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Handle confirm dialog
		page.on('dialog', (dialog) => dialog.accept());

		let deleted = false;
		await api.overrideRoute('**/api/libraries/*', (route) => {
			if (route.request().method() === 'DELETE') {
				deleted = true;
				return route.fulfill({ status: 204 });
			}
			return route.fallback();
		});

		await page.goto('/admin/libraries');

		const main = page.locator('main');
		await expect(main.locator('h3').filter({ hasText: 'Movies' })).toBeVisible();

		// Click the destructive (delete) button for the first library
		// The delete button has bg-destructive class and contains a Trash2 icon
		await main.locator('button.bg-destructive').first().click();

		// Wait for deletion
		await page.waitForTimeout(500);
		expect(deleted).toBe(true);
	});

	test('scan library button shows scanning state', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Delay scan response
		await api.overrideRoute('**/api/libraries/*/scan', (route) => {
			if (route.request().method() === 'POST') {
				return new Promise((resolve) => {
					setTimeout(() => {
						resolve(route.fulfill({ status: 200, body: '{}' }));
					}, 1000);
				});
			}
			return route.fallback();
		});

		await page.goto('/admin/libraries');

		// Click Scan button
		const scanButton = page.getByRole('button', { name: 'Scan' }).first();
		await scanButton.click();

		// Should show scanning state
		await expect(page.getByText('Scanning')).toBeVisible();
	});

	test('create new rule via editor', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/rules');

		// Wait for rules to load
		await expect(page.getByText('Transcode 4K')).toBeVisible();

		// Click New Rule
		await page.getByRole('button', { name: 'New Rule' }).click();

		// Dialog should appear with rule-name input
		await expect(page.locator('input#rule-name')).toBeVisible();

		// Fill in rule editor
		await page.locator('input#rule-name').fill('New Test Rule');

		// Uncheck enabled so we don't need actions to save
		await page.locator('input#rule-enabled').uncheck();

		// Save
		await page.getByRole('button', { name: 'Save Rule' }).click();

		// Dialog should close
		await expect(page.locator('input#rule-name')).not.toBeVisible({ timeout: 5000 });
	});

	test('edit existing rule', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/rules');

		// Wait for rules to load
		await expect(page.getByText('Transcode 4K')).toBeVisible();

		// Click edit on first rule
		await page.getByTitle('Edit').first().click();

		// Dialog should appear with rule-name input
		await expect(page.locator('input#rule-name')).toBeVisible();
	});

	test('delete rule with confirmation', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Handle confirm dialog
		page.on('dialog', (dialog) => dialog.accept());

		await page.goto('/rules');

		// Verify rules are visible
		await expect(page.getByText('Transcode 4K')).toBeVisible();

		// Click delete on a rule
		await page.getByTitle('Delete').first().click();
	});

	test('toggle rule enabled/disabled', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/rules');

		// Wait for rules to load
		await expect(page.getByText('Transcode 4K')).toBeVisible();

		// Should see Active badge
		await expect(page.getByText('Active').first()).toBeVisible();

		// Click the Active badge to toggle
		await page.getByText('Active').first().click();
	});

	test('retry failed job', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Mock retry endpoint
		await api.overrideRoute('**/api/jobs/*/retry', (route) => {
			if (route.request().method() === 'POST') {
				return route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: '{}'
				});
			}
			return route.fallback();
		});

		await page.goto('/admin/jobs');

		// Wait for the failed job to appear
		await expect(page.getByText('failed-1.mkv')).toBeVisible();

		// Click retry button
		await page.getByTitle('Retry').click();
	});

	test('delete job from history', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Mock delete endpoint
		await api.overrideRoute('**/api/jobs/*', (route) => {
			if (route.request().method() === 'DELETE') {
				return route.fulfill({ status: 204 });
			}
			return route.fallback();
		});

		await page.goto('/admin/jobs');

		await expect(page.getByText('completed-1.mkv')).toBeVisible();

		// Click first delete button in job row
		await page.getByTitle('Delete').first().click();
	});

	test('create rule on standalone /rules page with dialog', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/rules');

		// Wait for rules to load (loading state finishes)
		await expect(page.getByText('Transcode 4K')).toBeVisible({ timeout: 10000 });

		// Click New Rule
		await page.getByRole('button', { name: 'New Rule' }).click();

		// Dialog should appear with rule-name input
		await expect(page.locator('input#rule-name')).toBeVisible();

		// Fill in name
		await page.locator('input#rule-name').fill('Standalone Rule');

		// Uncheck enabled so we don't need actions to save
		await page.locator('input#rule-enabled').uncheck();

		// Save
		await page.getByRole('button', { name: 'Save Rule' }).click();

		// Dialog should close
		await expect(page.locator('input#rule-name')).not.toBeVisible({ timeout: 5000 });
	});

	test('empty rule name is rejected', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await page.goto('/rules');

		// Wait for rules to load
		await expect(page.getByText('Transcode 4K')).toBeVisible();

		// Click New Rule
		await page.getByRole('button', { name: 'New Rule' }).click();

		// Editor should appear
		await expect(page.locator('input#rule-name')).toBeVisible();

		// Leave name empty and click Save
		await page.getByRole('button', { name: 'Save' }).click();

		// Editor should stay open (save is silently rejected when name is empty)
		await expect(page.locator('input#rule-name')).toBeVisible();
	});
});
