import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mockFetchResponse, mockFetchError } from '../setup';
import {
  formatBytes,
  formatDuration,
  formatJobSource,
  getHealth,
  getStats,
  getJobs,
  getJob,
  retryJob,
  deleteJob,
  getQueue,
  getHistory,
  testArrConnection,
  getTools,
  ApiError,
} from '$lib/api';
import type { Job, JobStats, HealthResponse, ToolStatus } from '$lib/types';

describe('formatBytes', () => {
  it('formats 0 bytes', () => {
    expect(formatBytes(0)).toBe('0 B');
  });

  it('formats bytes', () => {
    expect(formatBytes(500)).toBe('500 B');
  });

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
  });

  it('formats megabytes', () => {
    expect(formatBytes(1048576)).toBe('1 MB');
    expect(formatBytes(1572864)).toBe('1.5 MB');
  });

  it('formats gigabytes', () => {
    expect(formatBytes(1073741824)).toBe('1 GB');
    expect(formatBytes(5368709120)).toBe('5 GB');
  });

  it('formats terabytes', () => {
    expect(formatBytes(1099511627776)).toBe('1 TB');
  });
});

describe('formatDuration', () => {
  it('formats seconds only', () => {
    expect(formatDuration(1000)).toBe('1s');
    expect(formatDuration(30000)).toBe('30s');
  });

  it('formats minutes and seconds', () => {
    expect(formatDuration(65000)).toBe('1m 5s');
    expect(formatDuration(120000)).toBe('2m 0s');
  });

  it('formats hours and minutes', () => {
    expect(formatDuration(3600000)).toBe('1h 0m');
    expect(formatDuration(3665000)).toBe('1h 1m');
    expect(formatDuration(7200000)).toBe('2h 0m');
  });
});

describe('formatJobSource', () => {
  it('formats webhook source', () => {
    const source = { webhook: { arr_name: 'radarr' } };
    expect(formatJobSource(source)).toBe('Webhook (radarr)');
  });

  it('formats watcher source', () => {
    const source = { watcher: { watch_path: '/movies' } };
    expect(formatJobSource(source)).toBe('File Watcher');
  });

  it('formats manual source', () => {
    expect(formatJobSource('manual')).toBe('Manual');
  });

  it('formats api source', () => {
    expect(formatJobSource('api')).toBe('Api');
  });
});

describe('API client functions', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  describe('getHealth', () => {
    it('returns health data on success', async () => {
      const healthData: HealthResponse = {
        status: 'healthy',
        version: '0.1.0',
        stats: {
          total_processed: 100,
          success_rate: 95.5,
        },
      };

      mockFetchResponse(healthData);

      const result = await getHealth();
      expect(result).toEqual(healthData);
      expect(fetch).toHaveBeenCalledWith('/api/health', expect.any(Object));
    });

    it('throws ApiError on failure', async () => {
      mockFetchError(500, 'Internal Server Error');

      await expect(getHealth()).rejects.toThrow(ApiError);
    });
  });

  describe('getStats', () => {
    it('returns stats data on success', async () => {
      const statsData: JobStats = {
        total_processed: 50,
        successful: 45,
        failed: 5,
        total_bytes_processed: 1073741824,
        rules_matched: { 'DV Conversion': 20, 'Remux': 30 },
      };

      mockFetchResponse(statsData);

      const result = await getStats();
      expect(result).toEqual(statsData);
    });
  });

  describe('getJobs', () => {
    it('returns jobs list on success', async () => {
      const jobs: Job[] = [
        {
          id: 'job-1',
          file_path: '/movies/test.mkv',
          file_name: 'test.mkv',
          status: 'running',
          rule_name: 'Test Rule',
          progress: 50,
          current_step: 'Processing',
          error: null,
          created_at: '2024-01-01T00:00:00Z',
          started_at: '2024-01-01T00:01:00Z',
          completed_at: null,
          source: 'api',
        },
      ];

      mockFetchResponse(jobs);

      const result = await getJobs();
      expect(result).toEqual(jobs);
    });

    it('includes query parameters', async () => {
      mockFetchResponse([]);

      await getJobs({ status: 'running', limit: 10, offset: 5 });

      expect(fetch).toHaveBeenCalledWith(
        '/api/jobs?status=running&limit=10&offset=5',
        expect.any(Object)
      );
    });
  });

  describe('getJob', () => {
    it('returns single job on success', async () => {
      const job: Job = {
        id: 'job-1',
        file_path: '/movies/test.mkv',
        file_name: 'test.mkv',
        status: 'completed',
        rule_name: 'Test Rule',
        progress: 100,
        current_step: null,
        error: null,
        created_at: '2024-01-01T00:00:00Z',
        started_at: '2024-01-01T00:01:00Z',
        completed_at: '2024-01-01T00:05:00Z',
        source: 'api',
      };

      mockFetchResponse(job);

      const result = await getJob('job-1');
      expect(result).toEqual(job);
    });

    it('throws on not found', async () => {
      mockFetchError(404, 'Not Found');

      await expect(getJob('nonexistent')).rejects.toThrow(ApiError);
    });
  });

  describe('retryJob', () => {
    it('returns new job on success', async () => {
      const job: Job = {
        id: 'job-2',
        file_path: '/movies/test.mkv',
        file_name: 'test.mkv',
        status: 'queued',
        rule_name: null,
        progress: 0,
        current_step: null,
        error: null,
        created_at: '2024-01-01T00:10:00Z',
        started_at: null,
        completed_at: null,
        source: 'api',
      };

      mockFetchResponse(job);

      const result = await retryJob('job-1');
      expect(result).toEqual(job);
      expect(fetch).toHaveBeenCalledWith('/api/jobs/job-1/retry', expect.objectContaining({ method: 'POST' }));
    });
  });

  describe('deleteJob', () => {
    it('succeeds on valid deletion', async () => {
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        ok: true,
        json: async () => ({}),
      });

      await expect(deleteJob('job-1')).resolves.toBeUndefined();
      expect(fetch).toHaveBeenCalledWith('/api/jobs/job-1', expect.objectContaining({ method: 'DELETE' }));
    });
  });

  describe('getQueue', () => {
    it('returns queued jobs', async () => {
      const jobs: Job[] = [
        {
          id: 'job-1',
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
        },
      ];

      mockFetchResponse(jobs);

      const result = await getQueue();
      expect(result).toEqual(jobs);
    });
  });

  describe('getHistory', () => {
    it('returns history with default limit', async () => {
      mockFetchResponse([]);

      await getHistory();
      expect(fetch).toHaveBeenCalledWith('/api/history?limit=100', expect.any(Object));
    });

    it('returns history with custom limit', async () => {
      mockFetchResponse([]);

      await getHistory(50);
      expect(fetch).toHaveBeenCalledWith('/api/history?limit=50', expect.any(Object));
    });
  });

  describe('testArrConnection', () => {
    it('returns success result', async () => {
      mockFetchResponse({ success: true });

      const result = await testArrConnection('radarr');
      expect(result).toEqual({ success: true });
      expect(fetch).toHaveBeenCalledWith('/api/arrs/radarr/test', expect.objectContaining({ method: 'POST' }));
    });

    it('returns error result', async () => {
      mockFetchResponse({ success: false, error: 'Connection failed' });

      const result = await testArrConnection('radarr');
      expect(result).toEqual({ success: false, error: 'Connection failed' });
    });
  });

  describe('getTools', () => {
    it('returns tool statuses', async () => {
      const tools: ToolStatus[] = [
        { name: 'ffmpeg', available: true, version: '6.0', path: '/usr/bin/ffmpeg' },
        { name: 'mediainfo', available: false, version: null, path: null },
      ];

      mockFetchResponse(tools);

      const result = await getTools();
      expect(result).toEqual(tools);
    });
  });
});

describe('ApiError', () => {
  it('has correct properties', () => {
    const error = new ApiError(404, 'Not Found');
    expect(error.status).toBe(404);
    expect(error.message).toBe('Not Found');
    expect(error.name).toBe('ApiError');
  });
});
