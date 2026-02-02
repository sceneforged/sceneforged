import type { Job, AppEvent } from '$lib/types';
import { getJobs, getHistory } from '$lib/api';
import { subscribe as subscribeToEvents } from '$lib/services/events.svelte';

// Module-level state using Svelte 5 runes (singleton pattern)
let jobs = $state<Job[]>([]);
let history = $state<Job[]>([]);

/**
 * Handle incoming job events from the event service
 */
function handleJobEvent(event: AppEvent): void {
  // Only process job:* events
  if (!event.type.startsWith('job:')) {
    return;
  }

  switch (event.type) {
    case 'job:queued':
      jobs = [...jobs, event.job];
      break;

    case 'job:started':
      jobs = jobs.map((j) =>
        j.id === event.id ? { ...j, status: 'running' as const, rule_name: event.rule_name } : j
      );
      break;

    case 'job:progress':
      jobs = jobs.map((j) =>
        j.id === event.id ? { ...j, progress: event.progress, current_step: event.step } : j
      );
      break;

    case 'job:completed':
      // Remove from active jobs and add to history
      jobs = jobs.filter((j) => j.id !== event.job.id);
      history = [event.job, ...history].slice(0, 1000);
      break;

    case 'job:failed':
      jobs = jobs.map((j) =>
        j.id === event.id ? { ...j, status: 'failed' as const, error: event.error } : j
      );
      break;
  }
}

// Subscribe to admin events on module load
subscribeToEvents('admin', handleJobEvent);

/**
 * Create a readable store-like interface
 */
function createReadableStore<T>(getValue: () => T) {
  return {
    subscribe(callback: (value: T) => void) {
      // Immediately call with current value
      callback(getValue());

      // Use $effect.root to track changes and notify subscribers
      const cleanup = $effect.root(() => {
        $effect(() => {
          callback(getValue());
        });
      });

      return cleanup;
    },
  };
}

/**
 * Create a writable store-like interface
 */
function createWritableStore<T>(getValue: () => T, setValue: (v: T) => void) {
  return {
    ...createReadableStore(getValue),
    set: setValue,
    update: (fn: (v: T) => T) => setValue(fn(getValue())),
  };
}

/**
 * Active jobs store - provides Svelte store interface
 */
export const activeJobs = {
  ...createWritableStore(
    () => jobs,
    (v: Job[]) => {
      jobs = v;
    }
  ),

  async refresh() {
    jobs = await getJobs();
  },

  handleEvent: handleJobEvent,
};

/**
 * Job history store - provides Svelte store interface
 */
export const jobHistory = {
  ...createWritableStore(
    () => history,
    (v: Job[]) => {
      history = v;
    }
  ),

  async refresh(limit = 100) {
    history = await getHistory(limit);
  },

  addJob(job: Job) {
    history = [job, ...history].slice(0, 1000);
  },

  removeJob(id: string) {
    history = history.filter((j) => j.id !== id);
  },
};

/**
 * Derived store for running jobs
 */
export const runningJobs = createReadableStore(() => jobs.filter((j) => j.status === 'running'));

/**
 * Derived store for queued jobs
 */
export const queuedJobs = createReadableStore(() => jobs.filter((j) => j.status === 'queued'));
