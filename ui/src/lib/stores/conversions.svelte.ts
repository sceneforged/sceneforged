import type { ConversionJob, AppEvent } from '$lib/types.js';
import { getConversions, getConversion } from '$lib/api/index.js';

function createConversionsStore() {
	let activeConversions = $state<ConversionJob[]>([]);
	let conversionHistory = $state<ConversionJob[]>([]);

	const runningConversions = $derived(activeConversions.filter((j) => j.status === 'processing'));
	const queuedConversions = $derived(activeConversions.filter((j) => j.status === 'queued'));

	return {
		get activeConversions() {
			return activeConversions;
		},
		get conversionHistory() {
			return conversionHistory;
		},
		get runningConversions() {
			return runningConversions;
		},
		get queuedConversions() {
			return queuedConversions;
		},

		hasActiveConversionForItem(itemId: string): boolean {
			return activeConversions.some((j) => j.item_id === itemId);
		},

		async refresh() {
			try {
				const jobs = await getConversions();
				activeConversions = jobs.filter(
					(j) => j.status === 'queued' || j.status === 'processing'
				);
				conversionHistory = jobs.filter(
					(j) => j.status === 'completed' || j.status === 'failed'
				);
			} catch (e) {
				console.error('Failed to refresh conversions:', e);
			}
		},

		async handleEvent(event: AppEvent) {
			const { payload } = event;

			switch (payload.type) {
				case 'conversion_queued': {
					try {
						const job = await getConversion(payload.job_id);
						activeConversions = [...activeConversions, job];
					} catch (e) {
						console.error('Failed to fetch queued conversion:', e);
					}
					break;
				}

				case 'conversion_started':
					activeConversions = activeConversions.map((j) =>
						j.id === payload.job_id
							? { ...j, status: 'processing', started_at: new Date().toISOString() }
							: j
					);
					break;

				case 'conversion_progress':
					activeConversions = activeConversions.map((j) =>
						j.id === payload.job_id
							? {
									...j,
									progress_pct: payload.progress * 100,
									encode_fps: payload.encode_fps ?? j.encode_fps,
									eta_secs: payload.eta_secs ?? j.eta_secs,
									bitrate: payload.bitrate ?? j.bitrate,
									speed: payload.speed ?? j.speed,
									output_size: payload.total_size ?? j.output_size
								}
							: j
					);
					break;

				case 'conversion_completed': {
					const completed = activeConversions.find((j) => j.id === payload.job_id);
					activeConversions = activeConversions.filter((j) => j.id !== payload.job_id);
					if (completed) {
						conversionHistory = [
							{ ...completed, status: 'completed' },
							...conversionHistory
						];
					}
					break;
				}

				case 'conversion_failed': {
					const failed = activeConversions.find((j) => j.id === payload.job_id);
					activeConversions = activeConversions.filter((j) => j.id !== payload.job_id);
					if (failed) {
						conversionHistory = [
							{ ...failed, status: 'failed', error: payload.error },
							...conversionHistory
						];
					}
					break;
				}
			}
		}
	};
}

export const conversionsStore = createConversionsStore();
