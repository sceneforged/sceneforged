import type { Job, AppEvent } from '$lib/types.js';
import { getJobs } from '$lib/api/index.js';

function createJobsStore() {
	let activeJobs = $state<Job[]>([]);
	let jobHistory = $state<Job[]>([]);

	const runningJobs = $derived(activeJobs.filter((j) => j.status === 'running'));
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
					(j) => j.status === 'queued' || j.status === 'running'
				);
				jobHistory = result.jobs.filter(
					(j) => j.status === 'completed' || j.status === 'failed' || j.status === 'cancelled'
				);
			} catch (e) {
				console.error('Failed to refresh jobs:', e);
			}
		},

		handleEvent(event: AppEvent) {
			const { payload } = event;

			switch (payload.type) {
				case 'job_queued':
					activeJobs = [...activeJobs, payload.job];
					break;

				case 'job_started':
					activeJobs = activeJobs.map((j) =>
						j.id === payload.job_id
							? { ...j, status: 'running' as const, rule_name: payload.rule_name }
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

				case 'job_completed':
					activeJobs = activeJobs.filter((j) => j.id !== payload.job.id);
					jobHistory = [payload.job, ...jobHistory];
					break;

				case 'job_failed':
					activeJobs = activeJobs.map((j) =>
						j.id === payload.job_id
							? { ...j, status: 'failed' as const, error: payload.error }
							: j
					);
					break;
			}
		}
	};
}

export const jobsStore = createJobsStore();
