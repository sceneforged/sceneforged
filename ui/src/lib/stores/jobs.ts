import { writable, derived } from 'svelte/store';
import type { Job, JobEvent } from '$lib/types';
import { getJobs, getHistory, subscribeToEvents } from '$lib/api';

function createJobsStore() {
  const { subscribe, set, update } = writable<Job[]>([]);

  return {
    subscribe,
    set,
    update,

    async refresh() {
      const jobs = await getJobs();
      set(jobs);
    },

    handleEvent(event: JobEvent) {
      update((jobs) => {
        switch (event.type) {
          case 'queued':
            return [...jobs, event.job];

          case 'started':
            return jobs.map((j) =>
              j.id === event.id
                ? { ...j, status: 'running' as const, rule_name: event.rule_name }
                : j
            );

          case 'progress':
            return jobs.map((j) =>
              j.id === event.id ? { ...j, progress: event.progress, current_step: event.step } : j
            );

          case 'completed':
            return jobs.filter((j) => j.id !== event.job.id);

          case 'failed':
            return jobs.map((j) =>
              j.id === event.id ? { ...j, status: 'failed' as const, error: event.error } : j
            );

          default:
            return jobs;
        }
      });
    },
  };
}

function createHistoryStore() {
  const { subscribe, set, update } = writable<Job[]>([]);

  return {
    subscribe,
    set,

    async refresh(limit = 100) {
      const history = await getHistory(limit);
      set(history);
    },

    addJob(job: Job) {
      update((jobs) => [job, ...jobs].slice(0, 1000));
    },

    removeJob(id: string) {
      update((jobs) => jobs.filter((j) => j.id !== id));
    },
  };
}

export const activeJobs = createJobsStore();
export const jobHistory = createHistoryStore();

// Derived stores
export const queuedJobs = derived(activeJobs, ($jobs) => $jobs.filter((j) => j.status === 'queued'));

export const runningJobs = derived(activeJobs, ($jobs) =>
  $jobs.filter((j) => j.status === 'running')
);

// SSE connection management
let unsubscribe: (() => void) | null = null;

export function connectToEvents() {
  if (unsubscribe) return;

  unsubscribe = subscribeToEvents(
    (event) => {
      activeJobs.handleEvent(event);

      // Add completed/failed jobs to history
      if (event.type === 'completed') {
        jobHistory.addJob(event.job);
      }
    },
    (error) => {
      console.error('SSE connection error:', error);
    }
  );
}

export function disconnectFromEvents() {
  unsubscribe?.();
  unsubscribe = null;
}
