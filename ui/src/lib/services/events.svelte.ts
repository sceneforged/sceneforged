import type { AppEvent } from '$lib/types';

// Module-level state (singleton pattern using Svelte 5 runes)
let eventSource: EventSource | null = $state(null);
let connected = $state(false);
let reconnectAttempts = $state(0);

// Subscriber management
type EventFilter = 'all' | 'user' | 'admin';
const subscribers = new Map<EventFilter, Set<(event: AppEvent) => void>>();

// Initialize subscriber sets
subscribers.set('all', new Set());
subscribers.set('user', new Set());
subscribers.set('admin', new Set());

// Constants for reconnection
const MAX_RECONNECT_DELAY = 30000; // 30 seconds
const BASE_RECONNECT_DELAY = 1000; // 1 second

/**
 * Calculate exponential backoff delay with jitter
 */
function getReconnectDelay(): number {
  const delay = Math.min(
    BASE_RECONNECT_DELAY * Math.pow(2, reconnectAttempts),
    MAX_RECONNECT_DELAY
  );
  // Add jitter (0-25% of delay) to prevent thundering herd
  const jitter = delay * Math.random() * 0.25;
  return delay + jitter;
}

/**
 * Route an event to appropriate subscribers based on category
 */
function routeEvent(event: AppEvent): void {
  // Route to 'all' subscribers
  subscribers.get('all')?.forEach((cb) => {
    try {
      cb(event);
    } catch (e) {
      console.error('Error in event subscriber callback:', e);
    }
  });

  // Route to category-specific subscribers
  const categorySubscribers = subscribers.get(event.category);
  categorySubscribers?.forEach((cb) => {
    try {
      cb(event);
    } catch (e) {
      console.error('Error in event subscriber callback:', e);
    }
  });
}

/**
 * Connect to the SSE event stream
 */
export function connect(): void {
  // Avoid duplicate connections
  if (eventSource !== null) {
    return;
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
      // Clean up current connection
      eventSource?.close();
      eventSource = null;
      connected = false;

      // Attempt reconnection with exponential backoff
      reconnectAttempts++;
      const delay = getReconnectDelay();

      console.log(
        `SSE connection lost. Reconnecting in ${Math.round(delay / 1000)}s (attempt ${reconnectAttempts})`
      );

      setTimeout(attemptConnect, delay);
    };
  }

  attemptConnect();
}

/**
 * Disconnect from the SSE event stream
 */
export function disconnect(): void {
  if (eventSource !== null) {
    eventSource.close();
    eventSource = null;
  }
  connected = false;
  reconnectAttempts = 0;
}

/**
 * Subscribe to events with optional filtering
 * @param filter - 'all' for all events, 'user' for user-category events, 'admin' for admin-category events
 * @param callback - Function to call when a matching event is received
 * @returns Unsubscribe function
 */
export function subscribe(
  filter: EventFilter,
  callback: (event: AppEvent) => void
): () => void {
  const subscriberSet = subscribers.get(filter);
  if (!subscriberSet) {
    console.error(`Invalid event filter: ${filter}`);
    return () => {};
  }

  subscriberSet.add(callback);

  // Return unsubscribe function
  return () => {
    subscriberSet.delete(callback);
  };
}

/**
 * Get the current connection status
 */
export function getIsConnected(): boolean {
  return connected;
}

/**
 * Get the current number of reconnection attempts
 */
export function getReconnectAttempts(): number {
  return reconnectAttempts;
}
