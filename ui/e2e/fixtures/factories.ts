import type {
	Library,
	Item,
	MediaFile,
	Image,
	Job,
	Rule,
	DashboardStats,
	ToolInfo,
	ActionConfig
} from '../../src/lib/types';

let idCounter = 0;
function nextId(): string {
	return `test-${++idCounter}`;
}

export function resetIdCounter(): void {
	idCounter = 0;
}

export function createLibrary(overrides: Partial<Library> = {}): Library {
	const id = overrides.id ?? nextId();
	return {
		id,
		name: `Library ${id}`,
		media_type: 'movies',
		paths: ['/media/movies'],
		config: {},
		created_at: new Date().toISOString(),
		...overrides
	};
}

export function createMediaFile(overrides: Partial<MediaFile> = {}): MediaFile {
	const id = overrides.id ?? nextId();
	return {
		id,
		item_id: overrides.item_id ?? nextId(),
		file_path: `/media/movies/test-file-${id}.mkv`,
		file_name: `test-file-${id}.mkv`,
		file_size: 1_500_000_000,
		container: 'mkv',
		video_codec: 'hevc',
		audio_codec: 'aac',
		resolution_width: 1920,
		resolution_height: 1080,
		hdr_format: undefined,
		has_dolby_vision: false,
		dv_profile: undefined,
		role: 'source',
		profile: 'A',
		duration_secs: 7200,
		created_at: new Date().toISOString(),
		...overrides
	};
}

export function createImage(overrides: Partial<Image> = {}): Image {
	const id = overrides.id ?? nextId();
	return {
		id,
		item_id: overrides.item_id ?? nextId(),
		image_type: 'primary',
		path: `/images/${id}.jpg`,
		provider: 'tmdb',
		width: 500,
		height: 750,
		...overrides
	};
}

export function createItem(overrides: Partial<Item> = {}): Item {
	const id = overrides.id ?? nextId();
	return {
		id,
		library_id: overrides.library_id ?? nextId(),
		item_kind: 'movie',
		name: `Test Movie ${id}`,
		sort_name: undefined,
		year: 2024,
		overview: 'A test movie for e2e testing.',
		runtime_minutes: 120,
		community_rating: 7.5,
		provider_ids: {},
		parent_id: undefined,
		season_number: undefined,
		episode_number: undefined,
		images: [],
		media_files: [],
		created_at: new Date().toISOString(),
		updated_at: new Date().toISOString(),
		...overrides
	};
}

export function createJob(overrides: Partial<Job> = {}): Job {
	const id = overrides.id ?? nextId();
	return {
		id,
		file_path: `/media/movies/test-${id}.mkv`,
		file_name: `test-${id}.mkv`,
		status: 'completed',
		rule_name: 'Default Rule',
		progress: 100,
		current_step: undefined,
		error: undefined,
		source: undefined,
		retry_count: 0,
		max_retries: 3,
		priority: 0,
		created_at: new Date().toISOString(),
		started_at: new Date().toISOString(),
		completed_at: new Date().toISOString(),
		...overrides
	};
}

export function createRule(overrides: Partial<Rule> = {}): Rule {
	const id = overrides.id ?? nextId();
	return {
		id,
		name: `Rule ${id}`,
		enabled: true,
		priority: 1,
		match_conditions: {},
		actions: [{ type: 'transcode' } as ActionConfig],
		...overrides
	};
}

export function createToolInfo(overrides: Partial<ToolInfo> = {}): ToolInfo {
	return {
		name: 'ffmpeg',
		available: true,
		version: '6.1',
		path: '/usr/bin/ffmpeg',
		...overrides
	};
}

export function createDashboardStats(overrides: Partial<DashboardStats> = {}): DashboardStats {
	return {
		total_libraries: 2,
		total_items: 50,
		total_jobs: 100,
		active_jobs: 1,
		completed_jobs: 95,
		failed_jobs: 4,
		tools: [
			createToolInfo({ name: 'ffmpeg', available: true }),
			createToolInfo({ name: 'ffprobe', available: true }),
			createToolInfo({ name: 'mp4box', available: false, version: undefined, path: undefined })
		],
		...overrides
	};
}
