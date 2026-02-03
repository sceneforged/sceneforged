const API_BASE = '/api';

/**
 * Error thrown when an API request fails with a non-ok response.
 */
export class ApiError extends Error {
	status: number;
	code: string;
	requestId: string | null;

	constructor(status: number, code: string, message: string, requestId: string | null = null) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.code = code;
		this.requestId = requestId;
	}
}

interface CacheEntry {
	data: unknown;
	expires: number;
}

const DEFAULT_TTL = 30_000; // 30 seconds
const MAX_RETRIES = 3;
const RETRY_DELAYS = [500, 1000, 2000];

/**
 * HTTP client with caching, request deduplication, and automatic retry.
 */
class ApiClient {
	private cache: Map<string, CacheEntry> = new Map();
	private inflight: Map<string, Promise<unknown>> = new Map();

	/**
	 * GET request with optional caching and deduplication.
	 */
	async get<T>(endpoint: string, opts?: { ttl?: number; skipCache?: boolean }): Promise<T> {
		const ttl = opts?.ttl ?? DEFAULT_TTL;
		const skipCache = opts?.skipCache ?? false;

		// Check cache first
		if (!skipCache) {
			const cached = this.cache.get(endpoint);
			if (cached && cached.expires > Date.now()) {
				return cached.data as T;
			}
		}

		// Deduplicate inflight requests
		const existing = this.inflight.get(endpoint);
		if (existing) {
			return existing as Promise<T>;
		}

		const promise = this._fetch<T>(endpoint, { method: 'GET' }).then((data) => {
			// Cache the result
			this.cache.set(endpoint, { data, expires: Date.now() + ttl });
			this.inflight.delete(endpoint);
			return data;
		}).catch((err) => {
			this.inflight.delete(endpoint);
			throw err;
		});

		this.inflight.set(endpoint, promise);
		return promise;
	}

	/**
	 * POST request. Bypasses cache.
	 */
	async post<T>(endpoint: string, body?: unknown): Promise<T> {
		return this._fetch<T>(endpoint, {
			method: 'POST',
			body: body !== undefined ? JSON.stringify(body) : undefined
		});
	}

	/**
	 * PUT request. Bypasses cache.
	 */
	async put<T>(endpoint: string, body?: unknown): Promise<T> {
		return this._fetch<T>(endpoint, {
			method: 'PUT',
			body: body !== undefined ? JSON.stringify(body) : undefined
		});
	}

	/**
	 * DELETE request. Bypasses cache.
	 */
	async delete(endpoint: string): Promise<void> {
		await this._fetch<void>(endpoint, { method: 'DELETE' });
	}

	/**
	 * Invalidate cache entries whose keys contain the given pattern.
	 */
	invalidate(pattern: string): void {
		for (const key of this.cache.keys()) {
			if (key.includes(pattern)) {
				this.cache.delete(key);
			}
		}
	}

	/**
	 * Internal fetch with automatic retry, content-type header, and error handling.
	 */
	private async _fetch<T>(endpoint: string, options: RequestInit): Promise<T> {
		const url = `${API_BASE}${endpoint}`;
		const headers: Record<string, string> = {
			'Content-Type': 'application/json',
			...(options.headers as Record<string, string>)
		};

		let lastError: Error | null = null;

		for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
			try {
				const response = await fetch(url, {
					...options,
					headers,
					credentials: 'include'
				});

				if (!response.ok) {
					const requestId = response.headers.get('x-request-id');
					let body: string;
					try {
						body = await response.text();
					} catch {
						body = response.statusText;
					}

					let code = 'UNKNOWN';
					try {
						const parsed = JSON.parse(body);
						if (parsed.code) code = parsed.code;
						if (parsed.message) body = parsed.message;
					} catch {
						// body is already a plain string
					}

					throw new ApiError(response.status, code, body, requestId);
				}

				// Handle 204 No Content
				if (response.status === 204 || response.headers.get('content-length') === '0') {
					return undefined as T;
				}

				return (await response.json()) as T;
			} catch (err) {
				lastError = err as Error;

				// Don't retry client errors (4xx) or if we've exhausted attempts
				if (err instanceof ApiError && err.status >= 400 && err.status < 500) {
					throw err;
				}

				if (attempt < MAX_RETRIES - 1) {
					await new Promise((resolve) => setTimeout(resolve, RETRY_DELAYS[attempt]));
				}
			}
		}

		throw lastError!;
	}
}

export const api = new ApiClient();
