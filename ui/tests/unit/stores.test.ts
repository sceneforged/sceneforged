import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import type { Job, JobEvent } from '$lib/types';

// Mock the API module before importing stores
vi.mock('$lib/api', () => ({
  getJobs: vi.fn(),
  getHistory: vi.fn(),
  subscribeToEvents: vi.fn(() => () => {}),
}));

// Import after mocking
import { activeJobs, jobHistory, queuedJobs, runningJobs } from '$lib/stores/jobs';
import { getJobs, getHistory } from '$lib/api';

// Create a mock job factory
function createMockJob(overrides: Partial<Job> = {}): Job {
  return {
    id: `job-${Math.random().toString(36).slice(2)}`,
    file_path: '/movies/test.mkv',
    file_name: 'test.mkv',
    status: 'queued',
    rule_name: null,
    progress: 0,
    current_step: null,
    error: null,
    created_at: '2024-01-01T00:00:00Z',
    started_at: null,
    completed_at: null,
    source: 'api',
    ...overrides,
  };
}

describe('activeJobs store', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    activeJobs.set([]);
  });

  it('starts empty', () => {
    expect(get(activeJobs)).toEqual([]);
  });

  it('can set jobs directly', () => {
    const jobs = [createMockJob({ id: 'job-1' }), createMockJob({ id: 'job-2' })];
    activeJobs.set(jobs);
    expect(get(activeJobs)).toEqual(jobs);
  });

  describe('refresh', () => {
    it('fetches jobs from API', async () => {
      const jobs = [createMockJob({ id: 'job-1' })];
      vi.mocked(getJobs).mockResolvedValueOnce(jobs);

      await activeJobs.refresh();

      expect(getJobs).toHaveBeenCalled();
      expect(get(activeJobs)).toEqual(jobs);
    });
  });

  describe('handleEvent', () => {
    it('handles queued event', () => {
      const newJob = createMockJob({ id: 'new-job' });
      const event: JobEvent = { type: 'queued', job: newJob };

      activeJobs.handleEvent(event);

      expect(get(activeJobs)).toContainEqual(newJob);
    });

    it('handles started event', () => {
      const job = createMockJob({ id: 'job-1', status: 'queued' });
      activeJobs.set([job]);

      const event: JobEvent = { type: 'started', id: 'job-1', rule_name: 'Test Rule' };
      activeJobs.handleEvent(event);

      const updated = get(activeJobs)[0];
      expect(updated.status).toBe('running');
      expect(updated.rule_name).toBe('Test Rule');
    });

    it('handles progress event', () => {
      const job = createMockJob({ id: 'job-1', status: 'running', progress: 0 });
      activeJobs.set([job]);

      const event: JobEvent = { type: 'progress', id: 'job-1', progress: 50, step: 'Processing' };
      activeJobs.handleEvent(event);

      const updated = get(activeJobs)[0];
      expect(updated.progress).toBe(50);
      expect(updated.current_step).toBe('Processing');
    });

    it('handles completed event - removes job', () => {
      const job = createMockJob({ id: 'job-1', status: 'running' });
      activeJobs.set([job]);

      const completedJob = { ...job, status: 'completed' as const, progress: 100 };
      const event: JobEvent = { type: 'completed', job: completedJob };
      activeJobs.handleEvent(event);

      expect(get(activeJobs)).toEqual([]);
    });

    it('handles failed event', () => {
      const job = createMockJob({ id: 'job-1', status: 'running' });
      activeJobs.set([job]);

      const event: JobEvent = { type: 'failed', id: 'job-1', error: 'Test error' };
      activeJobs.handleEvent(event);

      const updated = get(activeJobs)[0];
      expect(updated.status).toBe('failed');
      expect(updated.error).toBe('Test error');
    });

    it('does not modify other jobs', () => {
      const job1 = createMockJob({ id: 'job-1', status: 'queued' });
      const job2 = createMockJob({ id: 'job-2', status: 'queued' });
      activeJobs.set([job1, job2]);

      const event: JobEvent = { type: 'started', id: 'job-1', rule_name: 'Test' };
      activeJobs.handleEvent(event);

      const jobs = get(activeJobs);
      expect(jobs[0].status).toBe('running');
      expect(jobs[1].status).toBe('queued');
    });
  });
});

describe('jobHistory store', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    jobHistory.set([]);
  });

  it('starts empty', () => {
    expect(get(jobHistory)).toEqual([]);
  });

  describe('refresh', () => {
    it('fetches history from API', async () => {
      const history = [
        createMockJob({ id: 'job-1', status: 'completed' }),
        createMockJob({ id: 'job-2', status: 'failed' }),
      ];
      vi.mocked(getHistory).mockResolvedValueOnce(history);

      await jobHistory.refresh();

      expect(getHistory).toHaveBeenCalledWith(100);
      expect(get(jobHistory)).toEqual(history);
    });

    it('accepts custom limit', async () => {
      vi.mocked(getHistory).mockResolvedValueOnce([]);

      await jobHistory.refresh(50);

      expect(getHistory).toHaveBeenCalledWith(50);
    });
  });

  describe('addJob', () => {
    it('adds job to beginning of list', () => {
      const existing = createMockJob({ id: 'existing' });
      jobHistory.set([existing]);

      const newJob = createMockJob({ id: 'new' });
      jobHistory.addJob(newJob);

      const history = get(jobHistory);
      expect(history[0].id).toBe('new');
      expect(history[1].id).toBe('existing');
    });

    it('limits history to 1000 entries', () => {
      const largeHistory = Array.from({ length: 1000 }, (_, i) => createMockJob({ id: `job-${i}` }));
      jobHistory.set(largeHistory);

      const newJob = createMockJob({ id: 'new' });
      jobHistory.addJob(newJob);

      const history = get(jobHistory);
      expect(history.length).toBe(1000);
      expect(history[0].id).toBe('new');
    });
  });

  describe('removeJob', () => {
    it('removes job by id', () => {
      const jobs = [
        createMockJob({ id: 'job-1' }),
        createMockJob({ id: 'job-2' }),
        createMockJob({ id: 'job-3' }),
      ];
      jobHistory.set(jobs);

      jobHistory.removeJob('job-2');

      const history = get(jobHistory);
      expect(history.length).toBe(2);
      expect(history.find((j) => j.id === 'job-2')).toBeUndefined();
    });
  });
});

describe('derived stores', () => {
  beforeEach(() => {
    activeJobs.set([]);
  });

  describe('queuedJobs', () => {
    it('filters only queued jobs', () => {
      const jobs = [
        createMockJob({ id: 'job-1', status: 'queued' }),
        createMockJob({ id: 'job-2', status: 'running' }),
        createMockJob({ id: 'job-3', status: 'queued' }),
      ];
      activeJobs.set(jobs);

      const queued = get(queuedJobs);
      expect(queued.length).toBe(2);
      expect(queued.every((j) => j.status === 'queued')).toBe(true);
    });
  });

  describe('runningJobs', () => {
    it('filters only running jobs', () => {
      const jobs = [
        createMockJob({ id: 'job-1', status: 'queued' }),
        createMockJob({ id: 'job-2', status: 'running' }),
        createMockJob({ id: 'job-3', status: 'running' }),
      ];
      activeJobs.set(jobs);

      const running = get(runningJobs);
      expect(running.length).toBe(2);
      expect(running.every((j) => j.status === 'running')).toBe(true);
    });
  });
});
