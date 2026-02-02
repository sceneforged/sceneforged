import { test, expect, Page } from '@playwright/test';

// Mock API responses
const mockData = {
  stats: {
    total_items: 100,
    storage_bytes: 1024000000,
    items_by_profile: { profile_a: 50, profile_b: 30, profile_c: 20 },
  },
  queue: { queued: 0, running: 0 },
  streams: [],
  jobs: [],
  history: [],
  rules: [
    {
      name: 'test_rule',
      enabled: true,
      priority: 100,
      match: {
        codecs: ['hevc'],
        containers: ['mkv'],
        hdr_formats: [],
        dolby_vision_profiles: [7],
        min_resolution: null,
        max_resolution: null,
        audio_codecs: [],
      },
      actions: [{ type: 'dv_convert', target_profile: 8 }],
    },
  ],
  tools: [
    { name: 'ffmpeg', available: true, version: '6.0', path: '/usr/bin/ffmpeg' },
    { name: 'mediainfo', available: true, version: '23.0', path: '/usr/bin/mediainfo' },
    { name: 'mkvmerge', available: true, version: '80.0', path: '/usr/bin/mkvmerge' },
    { name: 'dovi_tool', available: true, version: '2.0', path: '/usr/bin/dovi_tool' },
  ],
  arrs: [],
  jellyfins: [],
  authStatus: { auth_enabled: false, authenticated: true, username: null },
  libraries: [
    { id: 'lib-1', name: 'Movies', media_type: 'movies', paths: ['/media/movies'] },
    { id: 'lib-2', name: 'TV Shows', media_type: 'tvshows', paths: ['/media/tv'] },
  ],
  items: { items: [], total_count: 0, offset: 0, limit: 50 },
  dashboard: {
    stats: {
      total_items: 100,
      storage_bytes: 1024000000,
      items_by_profile: { profile_a: 50, profile_b: 30, profile_c: 20 },
    },
    queue: { queued: 0, running: 0 },
    streams: [],
  },
};

// Setup API mocks for all tests
async function setupMocks(page: Page) {
  // Intercept ALL API requests with a single handler
  await page.route('**/api/**', async (route) => {
    const url = route.request().url();

    // Match specific endpoints
    if (url.includes('/api/admin/dashboard')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.dashboard) });
    }
    if (url.includes('/api/stats')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.stats) });
    }
    if (url.includes('/api/jobs')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.jobs) });
    }
    if (url.includes('/api/history')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.history) });
    }
    if (url.includes('/api/config/rules')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.rules) });
    }
    if (url.includes('/api/tools')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.tools) });
    }
    if (url.includes('/api/config/arrs')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.arrs) });
    }
    if (url.includes('/api/config/jellyfins')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.jellyfins) });
    }
    if (url.includes('/api/auth/status')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.authStatus) });
    }
    if (url.includes('/api/libraries')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.libraries) });
    }
    if (url.includes('/api/items')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.items) });
    }
    if (url.includes('/api/events')) {
      // SSE endpoint - return empty response with proper headers
      return route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        headers: { 'Cache-Control': 'no-cache', 'Connection': 'keep-alive' },
        body: 'data: {}\n\n'
      });
    }
    if (url.includes('/api/health') || url.includes('/health')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'healthy', version: '0.1.0', stats: { total_processed: 0, success_rate: 0 } }) });
    }

    // Default: return empty array for any other API endpoint (safer default)
    return route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
  });
}

// Helper to check for JS errors
async function gotoWithErrorCheck(page: Page, url: string): Promise<string[]> {
  const errors: string[] = [];
  page.on('pageerror', (err) => errors.push(err.message));

  await page.goto(url);
  await page.waitForLoadState('domcontentloaded');
  // Give more time for SPA to hydrate and render
  await page.waitForTimeout(1000);

  return errors;
}

// Helper to find page heading (h1 with title text)
async function expectPageHeading(page: Page, title: string) {
  // Try multiple approaches to find the heading
  const heading = page.locator(`h1:has-text("${title}")`);
  await expect(heading).toBeVisible({ timeout: 10000 });
}

test.describe('Sceneforged UI - Page Loading', () => {
  test.beforeEach(async ({ page }) => {
    await setupMocks(page);
  });

  test('Home page loads without JS errors', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'Welcome to Sceneforged');
  });

  test('Admin dashboard loads without JS errors', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/admin');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'Admin Dashboard');
  });

  test('History page loads without JS errors', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/history');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'History');
  });

  test('Rules page loads without JS errors', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/rules');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'Rules');
  });

  test('Settings page loads without JS errors', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/settings');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'Settings');
  });
});

test.describe('Sceneforged UI - Content Rendering', () => {
  test.beforeEach(async ({ page }) => {
    await setupMocks(page);
  });

  test('Home page shows welcome message', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');
    await expect(page.getByText('Your personal media library')).toBeVisible({ timeout: 10000 });
  });

  test('Admin dashboard shows stats cards', async ({ page }) => {
    await page.goto('/admin');
    await page.waitForLoadState('domcontentloaded');
    await expect(page.getByText('Library Items')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Storage Used')).toBeVisible({ timeout: 10000 });
  });

  test('History shows table', async ({ page }) => {
    await page.goto('/history');
    await page.waitForLoadState('domcontentloaded');
    await expect(page.locator('table').first()).toBeVisible({ timeout: 10000 });
  });

  test('Rules shows rule cards', async ({ page }) => {
    await page.goto('/rules');
    await page.waitForLoadState('domcontentloaded');
    await expect(page.getByText('test_rule')).toBeVisible({ timeout: 10000 });
  });

  test('Rules shows match conditions', async ({ page }) => {
    await page.goto('/rules');
    await page.waitForLoadState('domcontentloaded');
    // Should display codec from match conditions
    await expect(page.getByText(/hevc/i).first()).toBeVisible({ timeout: 10000 });
  });

  test('Settings shows tools', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('domcontentloaded');
    await expect(page.getByText(/ffmpeg/i).first()).toBeVisible({ timeout: 10000 });
  });
});

test.describe('Sceneforged UI - Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await setupMocks(page);
  });

  // Helper to open mobile menu if needed
  async function openMobileMenuIfNeeded(page: Page) {
    const viewport = page.viewportSize();
    if (viewport && viewport.width < 768) {
      // Mobile viewport - click hamburger menu (last button in mobile header)
      // The mobile header is in a div.md\:hidden, inside it there's a header
      const mobileHeader = page.locator('.md\\:hidden header');
      const buttons = mobileHeader.locator('button');
      const menuButton = buttons.last();
      await menuButton.click();
      await page.waitForTimeout(300); // Wait for menu animation
    }
  }

  test('Navigate from home to admin pages', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));

    await page.goto('/');
    await expectPageHeading(page, 'Welcome to Sceneforged');

    // Open mobile menu if needed and navigate to Admin Dashboard
    await openMobileMenuIfNeeded(page);
    await page.getByRole('link', { name: /dashboard/i }).click();
    await expectPageHeading(page, 'Admin Dashboard');

    // Navigate to History
    await openMobileMenuIfNeeded(page);
    await page.getByRole('link', { name: /history/i }).click();
    await expectPageHeading(page, 'History');

    // Navigate to Settings
    await openMobileMenuIfNeeded(page);
    await page.getByRole('link', { name: /settings/i }).click();
    await expectPageHeading(page, 'Settings');

    // Navigate to Home
    await openMobileMenuIfNeeded(page);
    await page.getByRole('link', { name: /home/i }).click();
    await expectPageHeading(page, 'Welcome to Sceneforged');

    // Check no JS errors occurred
    expect(errors).toHaveLength(0);
  });

  test('Sidebar shows libraries when loaded', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');

    // Open mobile menu if needed
    await openMobileMenuIfNeeded(page);

    await expect(page.getByRole('link', { name: /movies/i })).toBeVisible({ timeout: 10000 });
    await expect(page.getByRole('link', { name: /tv shows/i })).toBeVisible({ timeout: 10000 });
  });
});

test.describe('Sceneforged UI - Direct URL Access (SPA)', () => {
  test.beforeEach(async ({ page }) => {
    await setupMocks(page);
  });

  test('Direct access to /admin works', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/admin');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'Admin Dashboard');
  });

  test('Direct access to /history works', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/history');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'History');
  });

  test('Direct access to /rules works', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/rules');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'Rules');
  });

  test('Direct access to /settings works', async ({ page }) => {
    const errors = await gotoWithErrorCheck(page, '/settings');
    expect(errors).toHaveLength(0);
    await expectPageHeading(page, 'Settings');
  });
});

test.describe('Sceneforged UI - Edge Cases', () => {
  test('Empty rules array renders without errors', async ({ page }) => {
    await page.route('**/api/**', async (route) => {
      const url = route.request().url();
      if (url.includes('/api/config/rules')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
      }
      if (url.includes('/api/auth/status')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.authStatus) });
      }
      if (url.includes('/api/libraries')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.libraries) });
      }
      if (url.includes('/api/events')) {
        return route.fulfill({
          status: 200,
          contentType: 'text/event-stream',
          headers: { 'Cache-Control': 'no-cache', 'Connection': 'keep-alive' },
          body: 'data: {}\n\n'
        });
      }
      return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
    });

    const errors = await gotoWithErrorCheck(page, '/rules');
    expect(errors).toHaveLength(0);
    await expect(page.getByText(/no rules/i)).toBeVisible();
  });

  test('Rule with empty match conditions renders', async ({ page }) => {
    const minimalRule = {
      name: 'minimal_rule',
      enabled: true,
      priority: 1,
      match: {
        codecs: [],
        containers: [],
        hdr_formats: [],
        dolby_vision_profiles: [],
        min_resolution: null,
        max_resolution: null,
        audio_codecs: [],
      },
      actions: [],
    };

    await page.route('**/api/**', async (route) => {
      const url = route.request().url();
      if (url.includes('/api/config/rules')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([minimalRule]) });
      }
      if (url.includes('/api/auth/status')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.authStatus) });
      }
      if (url.includes('/api/libraries')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.libraries) });
      }
      if (url.includes('/api/events')) {
        return route.fulfill({
          status: 200,
          contentType: 'text/event-stream',
          headers: { 'Cache-Control': 'no-cache', 'Connection': 'keep-alive' },
          body: 'data: {}\n\n'
        });
      }
      return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
    });

    const errors = await gotoWithErrorCheck(page, '/rules');
    expect(errors).toHaveLength(0);
    await expect(page.getByText('minimal_rule')).toBeVisible();
  });

  test('No libraries shows info message', async ({ page }) => {
    await page.route('**/api/**', async (route) => {
      const url = route.request().url();
      if (url.includes('/api/auth/status')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.authStatus) });
      }
      if (url.includes('/api/libraries')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
      }
      if (url.includes('/api/items')) {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(mockData.items) });
      }
      if (url.includes('/api/events')) {
        return route.fulfill({
          status: 200,
          contentType: 'text/event-stream',
          headers: { 'Cache-Control': 'no-cache', 'Connection': 'keep-alive' },
          body: 'data: {}\n\n'
        });
      }
      return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
    });

    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');

    // On mobile, need to open menu to see sidebar
    const viewport = page.viewportSize();
    if (viewport && viewport.width < 768) {
      const mobileHeader = page.locator('.md\\:hidden header');
      const menuButton = mobileHeader.locator('button').last();
      await menuButton.click();
      await page.waitForTimeout(300);
    }

    // Look for "No Libraries" link in navigation
    await expect(page.getByRole('link', { name: /no libraries/i })).toBeVisible({ timeout: 10000 });
  });
});

test.describe('Sceneforged UI - Error Handling', () => {
  test('Admin dashboard handles API failure gracefully', async ({ page }) => {
    await page.route('**/api/**', (route) => route.abort());

    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));

    await page.goto('/admin');
    await page.waitForLoadState('domcontentloaded');

    // Page should render (even with errors shown)
    await expect(page.locator('body')).toBeVisible();

    // Filter out network errors (expected when we abort requests)
    const jsErrors = errors.filter(
      (e) => !e.includes('fetch') && !e.includes('network') && !e.includes('Failed to fetch') && !e.includes('NetworkError')
    );
    expect(jsErrors).toHaveLength(0);
  });

  test('Rules page handles API failure gracefully', async ({ page }) => {
    await page.route('**/api/**', (route) => route.abort());

    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));

    await page.goto('/rules');
    await page.waitForLoadState('domcontentloaded');

    await expect(page.locator('body')).toBeVisible();

    const jsErrors = errors.filter(
      (e) => !e.includes('fetch') && !e.includes('network') && !e.includes('Failed to fetch') && !e.includes('NetworkError')
    );
    expect(jsErrors).toHaveLength(0);
  });
});
