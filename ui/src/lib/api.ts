import type {
  Job,
  JobStats,
  HealthResponse,
  Rule,
  ToolStatus,
  JobEvent,
  AuthStatus,
  Library,
  Item,
  ItemsPage,
  MediaFile,
  PlaybackInfo,
  UserItemData,
  DashboardResponse,
  StreamSession,
  LibraryStats,
} from './types';

const API_BASE = '/api';

// Store API key for programmatic access (optional)
let apiKey: string | null = null;

export function setApiKey(key: string | null) {
  apiKey = key;
}

class ApiError extends Error {
  constructor(
    public status: number,
    message: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function fetchApi<T>(endpoint: string, options?: RequestInit): Promise<T> {
  const url = `${API_BASE}${endpoint}`;
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options?.headers as Record<string, string>),
  };

  // Add API key if set
  if (apiKey) {
    headers['Authorization'] = `Bearer ${apiKey}`;
  }

  const response = await fetch(url, {
    ...options,
    headers,
    credentials: 'include', // Include cookies for session auth
  });

  if (!response.ok) {
    const message = await response.text();
    throw new ApiError(response.status, message || response.statusText);
  }

  // Handle 204 No Content responses (e.g., DELETE operations)
  if (response.status === 204 || response.headers.get('content-length') === '0') {
    return undefined as T;
  }

  return response.json();
}

// Health & Stats
export async function getHealth(): Promise<HealthResponse> {
  return fetchApi('/health');
}

export async function getStats(): Promise<JobStats> {
  return fetchApi('/stats');
}

// Jobs
export async function getJobs(params?: {
  status?: string;
  limit?: number;
  offset?: number;
}): Promise<Job[]> {
  const searchParams = new URLSearchParams();
  if (params?.status) searchParams.set('status', params.status);
  if (params?.limit) searchParams.set('limit', String(params.limit));
  if (params?.offset) searchParams.set('offset', String(params.offset));

  const query = searchParams.toString();
  return fetchApi(`/jobs${query ? `?${query}` : ''}`);
}

export async function getJob(id: string): Promise<Job> {
  return fetchApi(`/jobs/${id}`);
}

export async function retryJob(id: string): Promise<Job> {
  return fetchApi(`/jobs/${id}/retry`, { method: 'POST' });
}

export async function deleteJob(id: string): Promise<void> {
  await fetchApi(`/jobs/${id}`, { method: 'DELETE' });
}

// Queue
export async function getQueue(): Promise<Job[]> {
  return fetchApi('/queue');
}

// Submit a job manually
export async function submitJob(filePath: string): Promise<{ job_id: string; file_path: string }> {
  return fetchApi('/jobs/submit', {
    method: 'POST',
    body: JSON.stringify({ file_path: filePath }),
  });
}

// History
export async function getHistory(limit = 100): Promise<Job[]> {
  return fetchApi(`/history?limit=${limit}`);
}

export async function testArrConnection(
  name: string
): Promise<{ success: boolean; error?: string }> {
  return fetchApi(`/arrs/${name}/test`, { method: 'POST' });
}

// Tools
export async function getTools(): Promise<ToolStatus[]> {
  return fetchApi('/tools');
}

// SSE subscription
export function subscribeToEvents(
  onEvent: (event: JobEvent) => void,
  onError?: (error: Error) => void
): () => void {
  let eventSource: EventSource | null = null;
  let reconnectAttempts = 0;
  const maxReconnectAttempts = 5;
  const reconnectDelay = 1000;

  function connect() {
    eventSource = new EventSource(`${API_BASE}/events`);

    eventSource.onopen = () => {
      reconnectAttempts = 0;
    };

    eventSource.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        onEvent(data);
      } catch (e) {
        console.error('Failed to parse SSE event:', e);
      }
    };

    eventSource.onerror = () => {
      eventSource?.close();

      if (reconnectAttempts < maxReconnectAttempts) {
        reconnectAttempts++;
        setTimeout(connect, reconnectDelay * reconnectAttempts);
      } else {
        onError?.(new Error('Failed to connect to event stream'));
      }
    };
  }

  connect();

  // Return cleanup function
  return () => {
    eventSource?.close();
  };
}

// Helper to format job source for display
export function formatJobSource(source: Job['source']): string {
  if (typeof source === 'string') {
    return source.charAt(0).toUpperCase() + source.slice(1);
  }
  if ('webhook' in source) {
    return `Webhook (${source.webhook.arr_name})`;
  }
  if ('watcher' in source) {
    return 'File Watcher';
  }
  return 'Unknown';
}

// Helper to format bytes
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

// Helper to format duration
export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${seconds}s`;
}

// Config - Rules
interface ApiRule {
  name: string;
  enabled: boolean;
  priority: number;
  match: {
    codecs: string[];
    containers: string[];
    hdr_formats: string[];
    dolby_vision_profiles: number[];
    min_resolution: { width: number; height: number } | null;
    max_resolution: { width: number; height: number } | null;
    audio_codecs: string[];
  };
  actions: Rule['actions'];
}

export async function getConfigRules(): Promise<Rule[]> {
  const apiRules = await fetchApi<ApiRule[]>('/config/rules');
  // Transform API response: 'match' -> 'match_conditions'
  return apiRules.map((rule) => ({
    name: rule.name,
    enabled: rule.enabled,
    priority: rule.priority,
    match_conditions: rule.match,
    actions: rule.actions,
  }));
}

function ruleToApiFormat(rule: Omit<Rule, 'normalized'>): Omit<ApiRule, 'normalized'> {
  return {
    name: rule.name,
    enabled: rule.enabled,
    priority: rule.priority,
    match: rule.match_conditions,
    actions: rule.actions,
  };
}

export async function createRule(rule: Omit<Rule, 'normalized'>): Promise<Rule> {
  const apiRule = await fetchApi<ApiRule>('/config/rules', {
    method: 'POST',
    body: JSON.stringify(ruleToApiFormat(rule)),
  });
  return {
    ...apiRule,
    match_conditions: apiRule.match,
  };
}

export async function updateRule(name: string, rule: Omit<Rule, 'normalized'>): Promise<Rule> {
  const apiRule = await fetchApi<ApiRule>(`/config/rules/${encodeURIComponent(name)}`, {
    method: 'PUT',
    body: JSON.stringify(ruleToApiFormat(rule)),
  });
  return {
    ...apiRule,
    match_conditions: apiRule.match,
  };
}

export async function deleteRule(name: string): Promise<void> {
  await fetchApi(`/config/rules/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  });
}

// Config - Arrs
export interface ArrConfigResponse {
  name: string;
  type: 'radarr' | 'sonarr';
  url: string;
  enabled: boolean;
  auto_rescan: boolean;
  auto_rename: boolean;
}

export async function getConfigArrs(): Promise<ArrConfigResponse[]> {
  return fetchApi('/config/arrs');
}

export interface CreateArrRequest {
  name: string;
  type: 'radarr' | 'sonarr';
  url: string;
  api_key: string;
  enabled?: boolean;
  auto_rescan?: boolean;
  auto_rename?: boolean;
}

export async function createArr(arr: CreateArrRequest): Promise<ArrConfigResponse> {
  return fetchApi('/config/arrs', {
    method: 'POST',
    body: JSON.stringify(arr),
  });
}

export interface UpdateArrRequest {
  name?: string;
  type?: 'radarr' | 'sonarr';
  url?: string;
  api_key?: string;
  enabled?: boolean;
  auto_rescan?: boolean;
  auto_rename?: boolean;
}

export async function updateArr(name: string, arr: UpdateArrRequest): Promise<ArrConfigResponse> {
  return fetchApi(`/config/arrs/${encodeURIComponent(name)}`, {
    method: 'PUT',
    body: JSON.stringify(arr),
  });
}

export async function deleteArr(name: string): Promise<void> {
  await fetchApi(`/config/arrs/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  });
}

// Config - Jellyfins
export interface JellyfinConfigResponse {
  name: string;
  url: string;
  enabled: boolean;
}

export async function getConfigJellyfins(): Promise<JellyfinConfigResponse[]> {
  return fetchApi('/config/jellyfins');
}

export interface CreateJellyfinRequest {
  name: string;
  url: string;
  api_key: string;
  enabled?: boolean;
}

export async function createJellyfin(jellyfin: CreateJellyfinRequest): Promise<JellyfinConfigResponse> {
  return fetchApi('/config/jellyfins', {
    method: 'POST',
    body: JSON.stringify(jellyfin),
  });
}

export interface UpdateJellyfinRequest {
  name?: string;
  url?: string;
  api_key?: string;
  enabled?: boolean;
}

export async function updateJellyfin(name: string, jellyfin: UpdateJellyfinRequest): Promise<JellyfinConfigResponse> {
  return fetchApi(`/config/jellyfins/${encodeURIComponent(name)}`, {
    method: 'PUT',
    body: JSON.stringify(jellyfin),
  });
}

export async function deleteJellyfin(name: string): Promise<void> {
  await fetchApi(`/config/jellyfins/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  });
}

// Config Operations
export async function reloadConfig(): Promise<{ status: string; message: string }> {
  return fetchApi('/config/reload', { method: 'POST' });
}

export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

export async function validateConfig(config: {
  rules?: Rule[];
  arrs?: CreateArrRequest[];
  jellyfins?: CreateJellyfinRequest[];
}): Promise<ValidationResult> {
  return fetchApi('/config/validate', {
    method: 'POST',
    body: JSON.stringify(config),
  });
}

// Auth
export async function getAuthStatus(): Promise<AuthStatus> {
  return fetchApi('/auth/status');
}

export async function login(
  username: string,
  password: string
): Promise<{ success: boolean; message: string; expires_at?: number }> {
  return fetchApi('/auth/login', {
    method: 'POST',
    body: JSON.stringify({ username, password }),
  });
}

export async function logout(): Promise<void> {
  await fetchApi('/auth/logout', { method: 'POST' });
}

// Library API

export async function getLibraries(): Promise<Library[]> {
  return fetchApi('/libraries');
}

export async function getLibrary(libraryId: string): Promise<Library | null> {
  return fetchApi(`/libraries/${libraryId}`);
}

export interface CreateLibraryRequest {
  name: string;
  media_type: 'movies' | 'tvshows' | 'music';
  paths: string[];
}

export async function createLibrary(data: CreateLibraryRequest): Promise<Library> {
  return fetchApi('/libraries', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function deleteLibrary(libraryId: string): Promise<void> {
  await fetchApi(`/libraries/${libraryId}`, { method: 'DELETE' });
}

export async function scanLibrary(libraryId: string): Promise<void> {
  await fetchApi(`/libraries/${libraryId}/scan`, { method: 'POST' });
}

export async function getItems(params?: {
  library_id?: string;
  parent_id?: string;
  item_kind?: string;
  search?: string;
  limit?: number;
  offset?: number;
  filter?: 'continue_watching' | 'recently_added' | 'favorites';
}): Promise<ItemsPage> {
  const searchParams = new URLSearchParams();
  if (params?.library_id) searchParams.set('library_id', params.library_id);
  if (params?.parent_id) searchParams.set('parent_id', params.parent_id);
  if (params?.item_kind) searchParams.set('item_kind', params.item_kind);
  if (params?.search) searchParams.set('search', params.search);
  if (params?.limit) searchParams.set('limit', String(params.limit));
  if (params?.offset) searchParams.set('offset', String(params.offset));
  if (params?.filter) searchParams.set('filter', params.filter);

  const query = searchParams.toString();
  return fetchApi(`/items${query ? `?${query}` : ''}`);
}

export async function getItem(itemId: string): Promise<Item> {
  return fetchApi(`/items/${itemId}`);
}

export async function getItemFiles(itemId: string): Promise<MediaFile[]> {
  return fetchApi(`/items/${itemId}/files`);
}

export async function searchItems(query: string, limit = 20): Promise<Item[]> {
  return fetchApi(`/search?q=${encodeURIComponent(query)}&limit=${limit}`);
}

// Playback API

export async function getPlaybackInfo(itemId: string): Promise<PlaybackInfo> {
  // Request web-only sources (Profile B/universal) for browser playback
  return fetchApi(`/playback/${itemId}/info?web_only=true`);
}

export async function updatePlaybackPosition(
  itemId: string,
  positionTicks: number
): Promise<void> {
  await fetchApi(`/playback/${itemId}/progress`, {
    method: 'POST',
    body: JSON.stringify({ position_ticks: positionTicks }),
  });
}

export async function markPlayed(itemId: string): Promise<void> {
  await fetchApi(`/playback/${itemId}/played`, { method: 'POST' });
}

export async function markUnplayed(itemId: string): Promise<void> {
  await fetchApi(`/playback/${itemId}/unplayed`, { method: 'POST' });
}

export async function toggleFavorite(itemId: string): Promise<UserItemData> {
  return fetchApi(`/playback/${itemId}/favorite`, { method: 'POST' });
}

// Streaming URLs (not fetched via API, just constructed)

export function getHlsMasterUrl(mediaFileId: string): string {
  return `/stream/${mediaFileId}/master.m3u8`;
}

export function getDirectStreamUrl(itemId: string): string {
  return `/direct/item/${itemId}`;
}

// Helper to format runtime ticks (100-nanosecond intervals) to human readable
export function formatRuntime(ticks: number | null): string {
  if (!ticks) return '';
  const totalSeconds = Math.floor(ticks / 10_000_000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

// Helper to format ticks to mm:ss or hh:mm:ss
export function formatTimestamp(ticks: number): string {
  const totalSeconds = Math.floor(ticks / 10_000_000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  const pad = (n: number) => n.toString().padStart(2, '0');

  if (hours > 0) {
    return `${hours}:${pad(minutes)}:${pad(seconds)}`;
  }
  return `${minutes}:${pad(seconds)}`;
}

// Path Browsing
export interface DirEntry {
  name: string;
  path: string;
  is_dir: boolean;
}

export async function browsePaths(path: string = '/', search?: string): Promise<DirEntry[]> {
  const params = new URLSearchParams({ path });
  if (search) params.set('search', search);
  return fetchApi(`/config/browse?${params}`);
}

// Admin API
export async function getAdminDashboard(): Promise<DashboardResponse> {
  return fetchApi('/admin/dashboard');
}

export async function getAdminStreams(): Promise<StreamSession[]> {
  return fetchApi('/admin/streams');
}

export async function getAdminStats(): Promise<LibraryStats> {
  return fetchApi('/admin/stats');
}

// Conversion API

export interface ConversionOptionsResponse {
  current_profiles: string[];
  viable_targets: string[];
}

export async function getConversionOptions(itemId: string): Promise<ConversionOptionsResponse> {
  return fetchApi(`/items/${itemId}/conversion`);
}

export interface ConvertItemRequest {
  target_profiles: string[];
}

export interface ConvertItemResponse {
  job_ids: string[];
}

export async function convertItem(itemId: string, targetProfiles: string[]): Promise<ConvertItemResponse> {
  return fetchApi(`/items/${itemId}/convert`, {
    method: 'POST',
    body: JSON.stringify({ target_profiles: targetProfiles }),
  });
}

export interface BatchConvertRequest {
  item_ids: string[];
  target_profile: string;
}

export interface BatchConvertResponse {
  job_ids: string[];
}

export async function batchConvert(itemIds: string[], targetProfile: 'A' | 'B' | 'C'): Promise<BatchConvertResponse> {
  return fetchApi('/conversions/batch', {
    method: 'POST',
    body: JSON.stringify({ item_ids: itemIds, target_profile: targetProfile }),
  });
}

export async function batchDvConvert(itemIds: string[]): Promise<BatchConvertResponse> {
  return fetchApi('/conversions/dv-batch', {
    method: 'POST',
    body: JSON.stringify({ item_ids: itemIds }),
  });
}

// Conversion Config
export interface ConversionConfig {
  auto_convert_dv_p7_to_p8: boolean;
}

export async function getConversionConfig(): Promise<ConversionConfig> {
  return fetchApi('/config/conversion');
}

export async function updateConversionConfig(config: ConversionConfig): Promise<ConversionConfig> {
  return fetchApi('/config/conversion', {
    method: 'PUT',
    body: JSON.stringify(config),
  });
}

export { ApiError };
