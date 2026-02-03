/// <reference types="@testing-library/jest-dom" />
import '@testing-library/jest-dom/vitest';
import { vi, beforeEach } from 'vitest';

// Mock fetch for API tests
globalThis.fetch = vi.fn() as typeof fetch;

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

globalThis.EventSource = MockEventSource as unknown as typeof EventSource;

// Reset mocks between tests
beforeEach(() => {
  vi.resetAllMocks();
});

// Helper to mock successful fetch responses
export function mockFetchResponse<T>(data: T): void {
  (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
    ok: true,
    status: 200,
    headers: new Headers({ 'content-type': 'application/json' }),
    json: async () => data,
    text: async () => JSON.stringify(data),
  });
}

// Helper to mock failed fetch responses
export function mockFetchError(status: number, message: string): void {
  (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
    ok: false,
    status,
    text: async () => message,
    statusText: message,
  });
}
