/// <reference types="@testing-library/jest-dom" />
import '@testing-library/jest-dom/vitest';

// Mock fetch for API tests
global.fetch = vi.fn();

// Mock EventSource for SSE tests
class MockEventSource {
  url: string;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    // Simulate connection open
    setTimeout(() => {
      this.onopen?.();
    }, 0);
  }

  close() {
    // Cleanup
  }
}

global.EventSource = MockEventSource as unknown as typeof EventSource;

// Reset mocks between tests
beforeEach(() => {
  vi.resetAllMocks();
});

// Helper to mock successful fetch responses
export function mockFetchResponse<T>(data: T): void {
  (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
    ok: true,
    json: async () => data,
    text: async () => JSON.stringify(data),
  });
}

// Helper to mock failed fetch responses
export function mockFetchError(status: number, message: string): void {
  (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
    ok: false,
    status,
    text: async () => message,
    statusText: message,
  });
}
