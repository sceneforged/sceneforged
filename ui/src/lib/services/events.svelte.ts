import type { AppEvent } from '$lib/types.js';

type EventFilter = 'admin' | 'user' | 'all';

const MAX_RECONNECT_DELAY = 30_000; // 30 seconds
const BASE_RECONNECT_DELAY = 1_000; // 1 second

function createEventsService() {
	let eventSource: EventSource | null = null;
	let connected = $state(false);
	let reconnectAttempts = $state(0);
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

	// Subscriber management
	const subscribers = new Map<EventFilter, Set<(event: AppEvent) => void>>();
	subscribers.set('all', new Set());
	subscribers.set('user', new Set());
	subscribers.set('admin', new Set());

	function getReconnectDelay(): number {
		const delay = Math.min(
			BASE_RECONNECT_DELAY * Math.pow(2, reconnectAttempts),
			MAX_RECONNECT_DELAY
		);
		// Jitter: 0-25% of delay to prevent thundering herd
		const jitter = delay * Math.random() * 0.25;
		return delay + jitter;
	}

	function routeEvent(event: AppEvent): void {
		// Notify 'all' subscribers
		subscribers.get('all')?.forEach((cb) => {
			try {
				cb(event);
			} catch (e) {
				console.error('Error in event subscriber callback:', e);
			}
		});

		// Notify category-specific subscribers
		const categorySubscribers = subscribers.get(event.category);
		categorySubscribers?.forEach((cb) => {
			try {
				cb(event);
			} catch (e) {
				console.error('Error in event subscriber callback:', e);
			}
		});
	}

	function attemptConnect(): void {
		eventSource = new EventSource('/api/events');

		eventSource.onopen = () => {
			connected = true;
			reconnectAttempts = 0;
		};

		eventSource.onmessage = (event: MessageEvent) => {
			try {
				const data = JSON.parse(event.data) as AppEvent;
				routeEvent(data);
			} catch (e) {
				console.error('Failed to parse SSE event:', e);
			}
		};

		eventSource.onerror = () => {
			eventSource?.close();
			eventSource = null;
			connected = false;

			reconnectAttempts++;
			const delay = getReconnectDelay();

			console.log(
				`SSE connection lost. Reconnecting in ${Math.round(delay / 1000)}s (attempt ${reconnectAttempts})`
			);

			reconnectTimer = setTimeout(attemptConnect, delay);
		};
	}

	return {
		get connected() {
			return connected;
		},
		get reconnectAttempts() {
			return reconnectAttempts;
		},

		connect(): void {
			if (eventSource !== null) {
				return;
			}
			attemptConnect();
		},

		disconnect(): void {
			if (reconnectTimer !== null) {
				clearTimeout(reconnectTimer);
				reconnectTimer = null;
			}
			if (eventSource !== null) {
				eventSource.close();
				eventSource = null;
			}
			connected = false;
			reconnectAttempts = 0;
		},

		subscribe(
			filter: EventFilter,
			callback: (event: AppEvent) => void
		): () => void {
			const subscriberSet = subscribers.get(filter);
			if (!subscriberSet) {
				console.error(`Invalid event filter: ${filter}`);
				return () => {};
			}

			subscriberSet.add(callback);

			return () => {
				subscriberSet.delete(callback);
			};
		}
	};
}

export const eventsService = createEventsService();
