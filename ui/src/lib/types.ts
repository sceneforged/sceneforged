// Job processing types
export interface Job {
	id: string;
	file_path: string;
	file_name: string;
	status: 'queued' | 'processing' | 'completed' | 'failed' | 'cancelled';
	rule_name?: string;
	progress: number;
	current_step?: string;
	error?: string;
	source?: string;
	retry_count: number;
	max_retries: number;
	priority: number;
	created_at: string;
	started_at?: string;
	completed_at?: string;
}

// Conversion job types
export interface ConversionJob {
	id: string;
	item_id: string;
	item_name?: string;
	media_file_id?: string;
	source_media_file_id?: string;
	status: string;
	progress_pct: number;
	encode_fps?: number;
	eta_secs?: number;
	elapsed_secs?: number;
	error?: string;
	error_message?: string;
	source_video_codec?: string;
	source_audio_codec?: string;
	source_resolution?: string;
	source_container?: string;
	created_at: string;
	started_at?: string;
	completed_at?: string;
}

// Playback types
export interface PlaybackState {
	item_id: string;
	position_secs: number;
	completed: boolean;
	play_count: number;
	last_played_at: string;
}

export interface FavoriteState {
	item_id: string;
	created_at: string;
}

export interface UserData {
	playback: PlaybackState | null;
	is_favorite: boolean;
}

// Library types
export interface Library {
	id: string;
	name: string;
	media_type: string;
	paths: string[];
	config: Record<string, unknown>;
	created_at: string;
}

// Item types
export interface Item {
	id: string;
	library_id: string;
	item_kind: string;
	name: string;
	sort_name?: string;
	year?: number;
	overview?: string;
	runtime_minutes?: number;
	community_rating?: number;
	provider_ids?: Record<string, string>;
	parent_id?: string;
	season_number?: number;
	episode_number?: number;
	images?: Image[];
	media_files?: MediaFile[];
	created_at: string;
	updated_at: string;
}

// Media file attached to an item
export interface MediaFile {
	id: string;
	item_id: string;
	file_path: string;
	file_name: string;
	file_size: number;
	container?: string;
	video_codec?: string;
	audio_codec?: string;
	resolution_width?: number;
	resolution_height?: number;
	hdr_format?: string;
	has_dolby_vision: boolean;
	dv_profile?: string;
	role: string;
	profile: string;
	duration_secs?: number;
	created_at: string;
}

// Image/artwork for an item
export interface Image {
	id: string;
	item_id: string;
	image_type: string;
	path: string;
	provider?: string;
	width?: number;
	height?: number;
}

// Processing rule configuration
export interface Rule {
	id: string;
	name: string;
	enabled: boolean;
	priority: number;
	match_conditions: MatchConditions;
	actions: ActionConfig[];
}

export interface MatchConditions {
	codecs: string[];
	containers: string[];
	hdr_formats: string[];
	dolby_vision_profiles: number[];
	audio_codecs: string[];
	min_resolution: { width: number; height: number } | null;
	max_resolution: { width: number; height: number } | null;
}

export interface ActionConfig {
	type: string;
	[key: string]: unknown;
}

// Directory browsing
export interface DirEntry {
	name: string;
	path: string;
	is_dir: boolean;
}

// Library statistics
export interface LibraryStats {
	total_items: number;
	profile_a: number;
	profile_b: number;
	profile_c: number;
}

// Dashboard statistics
export interface DashboardStats {
	jobs: { total: number; queued: number; processing: number };
	event_bus: { recent_events: number };
}

// External tool availability
export interface ToolInfo {
	name: string;
	available: boolean;
	version?: string;
	path?: string;
}

// Server-sent event envelope
export interface AppEvent {
	id: string;
	timestamp: string;
	category: 'admin' | 'user';
	payload: EventPayload;
}

// Discriminated union for event payloads (matches backend EventPayload in sf-core/src/events.rs)
export type EventPayload =
	| { type: 'job_queued'; job_id: string }
	| { type: 'job_started'; job_id: string }
	| { type: 'job_progress'; job_id: string; progress: number; step: string }
	| { type: 'job_completed'; job_id: string }
	| { type: 'job_failed'; job_id: string; error: string }
	| { type: 'library_scan_started'; library_id: string }
	| { type: 'library_scan_progress'; library_id: string; files_found: number; files_queued: number }
	| { type: 'library_scan_complete'; library_id: string; files_found: number; files_queued: number; files_skipped: number; errors: number }
	| { type: 'library_created'; library_id: string; name: string }
	| { type: 'library_deleted'; library_id: string }
	| { type: 'item_added'; item_id: string }
	| { type: 'item_updated'; item_id: string }
	| { type: 'item_removed'; item_id: string }
	| { type: 'conversion_queued'; job_id: string }
	| { type: 'conversion_started'; job_id: string }
	| { type: 'conversion_progress'; job_id: string; progress: number; encode_fps?: number; eta_secs?: number }
	| { type: 'conversion_completed'; job_id: string }
	| { type: 'conversion_failed'; job_id: string; error: string }
	| { type: 'heartbeat' };

// Config types
export interface ArrConfig {
	name: string;
	type: string;
	url: string;
	api_key: string;
	enabled: boolean;
	auto_rescan: boolean;
	auto_rename: boolean;
}

export interface JellyfinConfig {
	name: string;
	url: string;
	api_key: string;
	enabled: boolean;
}

export interface ConversionConfig {
	auto_convert_on_scan: boolean;
	auto_convert_dv_p7_to_p8: boolean;
	video_crf: number;
	video_preset: string;
	audio_bitrate: string;
	adaptive_crf: boolean;
}

// Enriched playback/favorite responses (include item data)
export interface ContinueWatchingEntry {
	item: Item;
	position_secs: number;
	completed: boolean;
	play_count: number;
	last_played_at: string;
}

export interface FavoriteEntry {
	item: Item;
	created_at: string;
}
