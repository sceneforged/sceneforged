import type { Job, AppEvent } from '$lib/types.js';
import { getJobs, getJob } from '$lib/api/index.js';

function createJobsStore() {
	let activeJobs = $state<Job[]>([]);
	let jobHistory = $state<Job[]>([]);

	const runningJobs = $derived(activeJobs.filter((j) => j.status === 'processing'));
	const queuedJobs = $derived(activeJobs.filter((j) => j.status === 'queued'));

	return {
		get activeJobs() {
			return activeJobs;
		},
		get jobHistory() {
			return jobHistory;
		},
		get runningJobs() {
			return runningJobs;
		},
		get queuedJobs() {
			return queuedJobs;
		},

		async refresh() {
			try {
				const result = await getJobs();
				activeJobs = result.jobs.filter(
					(j) => j.status === 'queued' || j.status === 'processing'
				);
				jobHistory = result.jobs.filter(
					(j) => j.status === 'completed' || j.status === 'failed' || j.status === 'cancelled'
				);
			} catch (e) {
				console.error('Failed to refresh jobs:', e);
			}
		},

		async handleEvent(event: AppEvent) {
			const { payload } = event;

			switch (payload.type) {
				case 'job_queued': {
					try {
						const job = await getJob(payload.job_id);
						activeJobs = [...activeJobs, job];
					} catch (e) {
						console.error('Failed to fetch queued job:', e);
					}
					break;
				}

				case 'job_started':
					activeJobs = activeJobs.map((j) =>
						j.id === payload.job_id
							? { ...j, status: 'processing' as const }
							: j
					);
					break;

				case 'job_progress':
					activeJobs = activeJobs.map((j) =>
						j.id === payload.job_id
							? { ...j, progress: payload.progress, current_step: payload.step }
							: j
					);
					break;

				case 'job_completed': {
					const completed = activeJobs.find((j) => j.id === payload.job_id);
					activeJobs = activeJobs.filter((j) => j.id !== payload.job_id);
					if (completed) {
						jobHistory = [
							{ ...completed, status: 'completed' as const },
							...jobHistory
						];
					}
					break;
				}

				case 'job_failed': {
					const failed = activeJobs.find((j) => j.id === payload.job_id);
					activeJobs = activeJobs.filter((j) => j.id !== payload.job_id);
					if (failed) {
						jobHistory = [
							{ ...failed, status: 'failed' as const, error: payload.error },
							...jobHistory
						];
					}
					break;
				}
			}
		}
	};
}

export const jobsStore = createJobsStore();
