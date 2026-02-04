import {
	createLibrary,
	createItem,
	createMediaFile,
	createJob,
	createRule,
	createDashboardStats,
	createToolInfo,
	resetIdCounter
} from './factories';
import type { Library, Item, Job, Rule, DashboardStats, ToolInfo } from '../../src/lib/types';

export interface Scenario {
	libraries: Library[];
	items: Item[];
	jobs: { jobs: Job[]; total: number };
	dashboard: DashboardStats;
	rules: Rule[];
	tools: ToolInfo[];
}

export function emptyState(): Scenario {
	resetIdCounter();
	return {
		libraries: [],
		items: [],
		jobs: { jobs: [], total: 0 },
		dashboard: createDashboardStats({
			total_libraries: 0,
			total_items: 0,
			total_jobs: 0,
			active_jobs: 0,
			completed_jobs: 0,
			failed_jobs: 0
		}),
		rules: [],
		tools: [createToolInfo({ name: 'ffmpeg', available: true })]
	};
}

export function populatedState(): Scenario {
	resetIdCounter();

	const movieLib = createLibrary({ name: 'Movies', media_type: 'movies', paths: ['/media/movies'] });
	const tvLib = createLibrary({ name: 'TV Shows', media_type: 'tvshows', paths: ['/media/tv'] });

	const items: Item[] = [];
	for (let i = 0; i < 6; i++) {
		const item = createItem({
			library_id: movieLib.id,
			name: `Movie ${i + 1}`,
			year: 2020 + i
		});
		const mediaFile = createMediaFile({
			item_id: item.id,
			role: i < 3 ? 'universal' : 'source',
			profile: i < 3 ? 'B' : 'A'
		});
		item.media_files = [mediaFile];
		items.push(item);
	}

	for (let i = 0; i < 4; i++) {
		items.push(
			createItem({
				library_id: tvLib.id,
				item_kind: 'episode',
				name: `Episode ${i + 1}`,
				season_number: 1,
				episode_number: i + 1
			})
		);
	}

	const completedJobs = Array.from({ length: 5 }, (_, i) =>
		createJob({ status: 'completed', file_name: `completed-${i + 1}.mkv` })
	);
	const failedJob = createJob({
		status: 'failed',
		file_name: 'failed-1.mkv',
		error: 'Encoding failed'
	});
	const runningJob = createJob({
		status: 'running',
		file_name: 'running-1.mkv',
		progress: 45,
		current_step: 'Transcoding video',
		completed_at: undefined
	});

	const allJobs = [...completedJobs, failedJob, runningJob];

	const rules = [
		createRule({ name: 'Transcode 4K', priority: 10, enabled: true }),
		createRule({ name: 'Extract Subtitles', priority: 5, enabled: true }),
		createRule({ name: 'Legacy Format', priority: 1, enabled: false })
	];

	return {
		libraries: [movieLib, tvLib],
		items,
		jobs: { jobs: allJobs, total: allJobs.length },
		dashboard: createDashboardStats({
			total_libraries: 2,
			total_items: items.length,
			total_jobs: allJobs.length,
			active_jobs: 1,
			completed_jobs: 5,
			failed_jobs: 1
		}),
		rules,
		tools: [
			createToolInfo({ name: 'ffmpeg', available: true }),
			createToolInfo({ name: 'ffprobe', available: true })
		]
	};
}

export function paginatedState(): Scenario {
	resetIdCounter();

	const lib = createLibrary({ name: 'Big Library', media_type: 'movies', paths: ['/media/big'] });

	const items: Item[] = [];
	for (let i = 0; i < 30; i++) {
		const item = createItem({
			library_id: lib.id,
			name: `Paginated Movie ${i + 1}`,
			year: 2020 + (i % 5)
		});
		if (i < 5) {
			item.media_files = [
				createMediaFile({ item_id: item.id, role: 'universal', profile: 'B' })
			];
		}
		items.push(item);
	}

	return {
		libraries: [lib],
		items,
		jobs: { jobs: [], total: 0 },
		dashboard: createDashboardStats({ total_libraries: 1, total_items: 30 }),
		rules: [],
		tools: [createToolInfo({ name: 'ffmpeg', available: true })]
	};
}

export function authDisabledState(): Scenario {
	resetIdCounter();
	return {
		libraries: [],
		items: [],
		jobs: { jobs: [], total: 0 },
		dashboard: createDashboardStats({
			total_libraries: 0,
			total_items: 0,
			total_jobs: 0,
			active_jobs: 0,
			completed_jobs: 0,
			failed_jobs: 0
		}),
		rules: [],
		tools: [createToolInfo({ name: 'ffmpeg', available: true })]
	};
}

export function noWebCompatibleState(): Scenario {
	resetIdCounter();

	const lib = createLibrary({ name: 'Movies', media_type: 'movies' });

	const items = Array.from({ length: 4 }, (_, i) => {
		const item = createItem({
			library_id: lib.id,
			name: `Source Only Movie ${i + 1}`
		});
		item.media_files = [
			createMediaFile({ item_id: item.id, role: 'source', profile: 'A' })
		];
		return item;
	});

	return {
		libraries: [lib],
		items,
		jobs: { jobs: [], total: 0 },
		dashboard: createDashboardStats({ total_libraries: 1, total_items: 4 }),
		rules: [],
		tools: [createToolInfo({ name: 'ffmpeg', available: true })]
	};
}
