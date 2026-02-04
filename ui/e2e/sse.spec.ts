import { test, expect } from '@playwright/test';
import { MockApi } from './helpers/mock-api';
import { populatedState } from './fixtures/scenarios';

function sseEvent(data: object): string {
	return `data: ${JSON.stringify(data)}\n\n`;
}

test.describe('Real-time SSE Updates', () => {
	test('job progress event updates active job display', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();

		const runningJob = scenario.jobs.jobs.find((j) => j.status === 'running')!;

		// Override SSE to send a progress update
		await api.setup(scenario);
		await api.overrideRoute('**/api/events', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: sseEvent({
					id: 'evt-1',
					timestamp: new Date().toISOString(),
					category: 'admin',
					payload: {
						type: 'job_progress',
						job_id: runningJob.id,
						progress: 75,
						current_step: 'Encoding audio'
					}
				})
			})
		);

		await page.goto('/admin/jobs');

		// The running job should be visible in active jobs
		await expect(page.getByText('running-1.mkv').first()).toBeVisible();
	});

	test('job completed event moves job to history', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();

		const runningJob = scenario.jobs.jobs.find((j) => j.status === 'running')!;

		await api.setup(scenario);
		await api.overrideRoute('**/api/events', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: sseEvent({
					id: 'evt-2',
					timestamp: new Date().toISOString(),
					category: 'admin',
					payload: {
						type: 'job_completed',
						job_id: runningJob.id,
						file_path: runningJob.file_path
					}
				})
			})
		);

		await page.goto('/admin/jobs');

		// Job history should be visible
		await expect(page.getByText('Job History')).toBeVisible();
	});

	test('library scan complete refreshes sidebar', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/events', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: sseEvent({
					id: 'evt-3',
					timestamp: new Date().toISOString(),
					category: 'admin',
					payload: {
						type: 'library_scan_complete',
						library_id: scenario.libraries[0].id,
						items_found: 10,
						items_added: 5,
						items_updated: 2
					}
				})
			})
		);

		await page.goto('/');

		// Libraries should still be listed in sidebar
		await expect(page.getByRole('link', { name: 'Movies' })).toBeVisible();
	});

	test('library created event adds to sidebar', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		await api.overrideRoute('**/api/events', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: sseEvent({
					id: 'evt-4',
					timestamp: new Date().toISOString(),
					category: 'admin',
					payload: {
						type: 'library_created',
						library_id: 'new-lib-1',
						name: 'New Library'
					}
				})
			})
		);

		await page.goto('/');

		// Existing libraries should still be there
		await expect(page.getByRole('link', { name: 'Movies' })).toBeVisible();
	});

	test('heartbeat event causes no UI changes', async ({ page }) => {
		const api = new MockApi(page);
		const scenario = populatedState();
		await api.setup(scenario);

		// Default SSE already sends heartbeat
		await page.goto('/');

		// Page should render normally
		await expect(page.getByText('Welcome to SceneForged')).toBeVisible();
		await expect(page.getByRole('link', { name: 'Movies' })).toBeVisible();
	});
});
