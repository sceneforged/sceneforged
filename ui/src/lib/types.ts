// Job types matching Rust backend
export interface Job {
  id: string;
  file_path: string;
  file_name: string;
  status: JobStatus;
  rule_name: string | null;
  progress: number;
  current_step: string | null;
  error: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  source: JobSource;
}

export type JobStatus = 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';

export type JobSource =
  | { webhook: { arr_name: string } }
  | { watcher: { watch_path: string } }
  | 'manual'
  | 'api';

// Stats
export interface JobStats {
  total_processed: number;
  successful: number;
  failed: number;
  total_bytes_processed: number;
  rules_matched: Record<string, number>;
}

// Health check response
export interface HealthResponse {
  status: string;
  version: string;
  stats: {
    total_processed: number;
    success_rate: number;
  };
}

// Rule types
export interface Rule {
  name: string;
  enabled: boolean;
  priority: number;
  match_conditions: MatchConditions;
  actions: Action[];
}

export interface MatchConditions {
  codecs: string[];
  containers: string[];
  hdr_formats: string[];
  dolby_vision_profiles: number[];
  min_resolution: Resolution | null;
  max_resolution: Resolution | null;
  audio_codecs: string[];
}

export interface Resolution {
  width: number;
  height: number;
}

export type Action =
  | { type: 'dv_convert'; target_profile: number }
  | { type: 'remux'; container: string; keep_original: boolean }
  | { type: 'add_compat_audio'; source_codec: string; target_codec: string }
  | { type: 'strip_tracks'; track_types: string[]; languages: string[] }
  | { type: 'exec'; command: string; args: string[] };

// Arr connection types
export interface ArrConnection {
  name: string;
  type: 'radarr' | 'sonarr';
  url: string;
  enabled: boolean;
  status: 'connected' | 'disconnected' | 'error';
  error?: string;
}

// Tool status
export interface ToolStatus {
  name: string;
  available: boolean;
  version: string | null;
  path: string | null;
}

// SSE event types
export type JobEvent =
  | { type: 'queued'; job: Job }
  | { type: 'started'; id: string; rule_name: string }
  | { type: 'progress'; id: string; progress: number; step: string }
  | { type: 'completed'; job: Job }
  | { type: 'failed'; id: string; error: string };

// Auth types
export interface AuthStatus {
  auth_enabled: boolean;
  authenticated: boolean;
  username: string | null;
}

// Library types
export interface Library {
  id: string;
  name: string;
  media_type: MediaType;
  paths: string[];
  created_at: string;
  updated_at: string;
}

export type MediaType = 'movies' | 'tvshows' | 'music';

// Person metadata (actor, director, writer, etc.)
export interface Person {
  name: string;
  role: string | null;
  person_type: string; // 'Actor', 'Director', 'Writer', etc.
  image_url: string | null;
}

// Image types for artwork
export type ImageType = 'primary' | 'backdrop' | 'banner' | 'thumb' | 'logo';

// Image/artwork for an item
export interface ItemImage {
  id: string;
  item_id: string;
  image_type: ImageType;
  path: string;
  provider: string | null;
  width: number | null;
  height: number | null;
}

export interface Item {
  id: string;
  library_id: string;
  parent_id: string | null;
  item_kind: ItemKind;
  name: string;
  sort_name: string | null;
  original_title: string | null;
  file_path: string | null;
  container: string | null;
  video_codec: string | null;
  audio_codec: string | null;
  resolution: string | null;
  runtime_ticks: number | null;
  size_bytes: number | null;
  overview: string | null;
  tagline: string | null;
  genres: string[];
  tags: string[];
  studios: string[];
  // People (cast/crew) - optional, may not be present in all API responses
  people?: Person[];
  community_rating: number | null;
  critic_rating: number | null;
  production_year: number | null;
  premiere_date: string | null;
  official_rating: string | null;
  index_number: number | null;
  parent_index_number: number | null;
  hdr_type: string | null;
  dolby_vision_profile: string | null;
  has_profile_a: boolean;
  has_profile_b: boolean;
  has_profile_c: boolean;
  date_created: string;
  date_modified: string;
  // Images - optional, may be fetched separately
  images?: ItemImage[];
  // Provider IDs for external lookups
  provider_ids?: {
    tmdb?: string;
    imdb?: string;
    tvdb?: string;
  };
}

export type ItemKind =
  | 'movie'
  | 'series'
  | 'season'
  | 'episode'
  | 'audio'
  | 'audio_album'
  | 'audio_artist'
  | 'collection'
  | 'folder';

export interface MediaFile {
  id: string;
  item_id: string;
  role: FileRole;
  profile: Profile;
  can_be_profile_a: boolean;
  can_be_profile_b: boolean;
  file_path: string;
  file_size: number;
  container: string;
  video_codec: string | null;
  audio_codec: string | null;
  width: number | null;
  height: number | null;
  duration_ticks: number | null;
  bit_rate: number | null;
  is_hdr: boolean;
  serves_as_universal: boolean;
  has_faststart: boolean;
  keyframe_interval_secs: number | null;
  created_at: string;
}

export type FileRole = 'source' | 'universal' | 'extra';
export type Profile = 'A' | 'B' | 'C';

// Backend response format for playback info
export interface PlaybackInfo {
  item_id: string;
  media_sources: MediaSourceInfo[];
}

export interface MediaSourceInfo {
  id: string;
  file_path: string;
  container: string;
  size: number;
  duration_ticks: number | null;
  supports_direct_play: boolean;
  supports_direct_stream: boolean;
  supports_transcoding: boolean;
  video_codec: string | null;
  audio_codec: string | null;
  width: number | null;
  height: number | null;
  is_hdr: boolean;
  serves_as_universal: boolean;
  direct_stream_url: string | null;
  hls_url: string | null;
}

export interface UserItemData {
  item_id: string;
  user_id: string;
  played: boolean;
  play_count: number;
  is_favorite: boolean;
  playback_position_ticks: number | null;
  last_played_date: string | null;
}

export interface ItemsPage {
  items: Item[];
  total_count: number;
  offset: number;
  limit: number;
}

// Admin types
export interface DashboardResponse {
  stats: LibraryStats;
  streams: StreamSession[];
  queue: QueueSummary;
}

export interface LibraryStats {
  total_items: number;
  total_files: number;
  storage_bytes: number;
  items_by_profile: ProfileCounts;
}

export interface ProfileCounts {
  profile_a: number;
  profile_b: number;
  profile_c: number;
}

export interface StreamSession {
  id: string;
  client_ip: string;
  item_id: number;
  profile: string;
  started_at: string;
  duration_seconds: number;
}

export interface QueueSummary {
  queued: number;
  running: number;
}

// Conversion job (from database)
export interface ConversionJob {
  id: string;
  item_id: string;
  source_file_id: string;
  status: string;
  progress_pct: number;
  encode_fps: number | null;
  eta_secs: number | null;
  elapsed_secs: number | null;
  output_path: string | null;
  error_message: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  item_name: string | null;
  source_video_codec: string | null;
  source_audio_codec: string | null;
  source_resolution: string | null;
  source_container: string | null;
}

// Unified SSE event type with category routing
// Backend uses serde with #[serde(tag = "event_type", rename_all = "snake_case")]
// Category determines routing: 'admin' for job events, 'user' for library/item events
export type AppEvent =
  // Job events - admin only
  | { category: 'admin'; event_type: 'job_queued'; job: Job }
  | { category: 'admin'; event_type: 'job_started'; id: string; rule_name: string }
  | { category: 'admin'; event_type: 'job_progress'; id: string; progress: number; step: string }
  | { category: 'admin'; event_type: 'job_completed'; job: Job }
  | { category: 'admin'; event_type: 'job_failed'; id: string; error: string }
  // Library events - user (UI updates)
  | { category: 'user'; event_type: 'library_scan_started'; library_id: string }
  | { category: 'user'; event_type: 'library_scan_complete'; library_id: string; items_added: number }
  | { category: 'user'; event_type: 'library_created'; library: Library }
  | { category: 'user'; event_type: 'library_deleted'; library_id: string }
  // Item events - user (UI updates)
  | { category: 'user'; event_type: 'item_added'; item: Item }
  | { category: 'user'; event_type: 'item_updated'; item: Item }
  | { category: 'user'; event_type: 'item_removed'; item_id: string }
  | { category: 'user'; event_type: 'playback_available'; item_id: string }
  // Conversion job events - admin
  | { category: 'admin'; event_type: 'conversion_job_created'; job_id: string; item_id: string; status: string }
  | { category: 'admin'; event_type: 'conversion_job_progress'; job_id: string; item_id: string; progress_pct: number; encode_fps: number | null; eta_secs: number | null; elapsed_secs: number }
  | { category: 'admin'; event_type: 'conversion_job_completed'; job_id: string; item_id: string }
  | { category: 'admin'; event_type: 'conversion_job_failed'; job_id: string; item_id: string; error: string }
  | { category: 'admin'; event_type: 'conversion_job_cancelled'; job_id: string; item_id: string }
  // System events
  | { category: 'user'; event_type: 'heartbeat' };
