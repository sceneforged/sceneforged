<script lang="ts">
	import { onMount } from 'svelte';
	import { getJobs, retryJob, deleteJob } from '$lib/api/index.js';
	import type { Job } from '$lib/types.js';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import Badge from '$lib/components/ui/badge/Badge.svelte';
	import Button from '$lib/components/ui/button/Button.svelte';
	import Input from '$lib/components/ui/input/Input.svelte';
	import Progress from '$lib/components/ui/progress/Progress.svelte';
	import { jobsStore } from '$lib/stores/jobs.svelte.js';
	import {
		Activity,
		CheckCircle,
		XCircle,
		Clock,
		RefreshCw,
		ChevronLeft,
		ChevronRight,
		Search,
		Trash2,
		RotateCcw,
		Loader2
	} from 'lucide-svelte';

	let loading = $state(true);
	let error = $state<string | null>(null);
	let allJobs = $state<Job[]>([]);
	let totalJobs = $state(0);
	let globalFilter = $state('');
	let currentPage = $state(0);
	const pageSize = 25;

	// Filter jobs based on search
	const filteredJobs = $derived.by(() => {
		if (!globalFilter) return allJobs;
		const filter = globalFilter.toLowerCase();
		return allJobs.filter(
			(job) =>
				job.file_name?.toLowerCase().includes(filter) ||
				job.rule_name?.toLowerCase().includes(filter) ||
				job.status?.toLowerCase().includes(filter)
		);
	});

	const totalPages = $derived(Math.ceil(filteredJobs.length / pageSize));
	const paginatedJobs = $derived(
		filteredJobs.slice(currentPage * pageSize, (currentPage + 1) * pageSize)
	);

	async function loadData() {
		loading = true;
		error = null;
		try {
			const result = await getJobs({ limit: 500 });
			allJobs = result.jobs;
			totalJobs = result.total;
			await jobsStore.refresh();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load jobs';
		} finally {
			loading = false;
		}
	}

	async function handleRetry(job: Job) {
		try {
			await retryJob(job.id);
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to retry job';
		}
	}

	async function handleDelete(job: Job) {
		try {
			await deleteJob(job.id);
			allJobs = allJobs.filter((j) => j.id !== job.id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete job';
		}
	}

	function getStatusVariant(status: string): 'default' | 'destructive' | 'outline' {
		switch (status) {
			case 'completed':
				return 'default';
			case 'failed':
				return 'destructive';
			default:
				return 'outline';
		}
	}

	function getStatusClass(status: string): string {
		return status === 'completed' ? 'bg-green-500' : '';
	}

	onMount(() => {
		loadData();
	});
</script>

<svelte:head>
	<title>Jobs - Admin - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold">Jobs</h1>
		<Button variant="outline" size="sm" onclick={loadData} disabled={loading}>
			<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
			Refresh
		</Button>
	</div>

	{#if error}
		<div class="rounded-md bg-destructive/10 p-4 text-destructive">
			{error}
		</div>
	{/if}

	<!-- Active jobs from store -->
	{#if jobsStore.runningJobs.length > 0 || jobsStore.queuedJobs.length > 0}
		<Card class="border-blue-500/50">
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Activity class="h-5 w-5 animate-pulse text-blue-500" />
					Active Jobs
					<Badge variant="secondary">
						{jobsStore.runningJobs.length + jobsStore.queuedJobs.length}
					</Badge>
				</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="space-y-4">
					{#each jobsStore.runningJobs as job (job.id)}
						<div class="space-y-2 rounded-lg border p-4">
							<div class="flex items-center justify-between">
								<div>
									<p class="text-sm font-medium">{job.file_name}</p>
									<p class="text-xs text-muted">Rule: {job.rule_name ?? 'N/A'}</p>
								</div>
								<Badge variant="secondary" class="bg-blue-500 text-white">
									<Activity class="mr-1 h-3 w-3 animate-pulse" />
									Running
								</Badge>
							</div>
							{#if job.progress > 0}
								<div class="space-y-1">
									<div class="flex justify-between text-xs">
										<span class="text-muted">{job.current_step ?? 'Processing...'}</span>
										<span class="font-medium">{job.progress}%</span>
									</div>
									<Progress value={job.progress} max={100} />
								</div>
							{/if}
						</div>
					{/each}

					{#each jobsStore.queuedJobs as job (job.id)}
						<div class="flex items-center justify-between rounded-lg border p-3">
							<div>
								<p class="truncate text-sm font-medium">{job.file_name}</p>
								<p class="text-xs text-muted">Rule: {job.rule_name ?? 'N/A'}</p>
							</div>
							<Badge variant="outline">Queued</Badge>
						</div>
					{/each}
				</div>
			</CardContent>
		</Card>
	{/if}

	<!-- Job History -->
	<Card>
		<CardHeader>
			<div class="flex items-center justify-between">
				<CardTitle>Job History</CardTitle>
				<div class="relative w-64">
					<Search class="absolute left-2 top-2.5 h-4 w-4 text-muted" />
					<Input placeholder="Search jobs..." class="pl-8" bind:value={globalFilter} />
				</div>
			</div>
		</CardHeader>
		<CardContent>
			{#if loading && allJobs.length === 0}
				<div class="flex items-center justify-center py-12">
					<Loader2 class="h-6 w-6 animate-spin text-muted" />
				</div>
			{:else}
				<div class="rounded-md border">
					<table class="w-full">
						<thead>
							<tr class="border-b bg-muted/50">
								<th class="px-4 py-3 text-left text-sm font-medium">File</th>
								<th class="px-4 py-3 text-left text-sm font-medium">Status</th>
								<th class="px-4 py-3 text-left text-sm font-medium">Rule</th>
								<th class="px-4 py-3 text-left text-sm font-medium">Completed</th>
								<th class="w-24 px-4 py-3 text-left text-sm font-medium">Actions</th>
							</tr>
						</thead>
						<tbody>
							{#if paginatedJobs.length === 0}
								<tr>
									<td colspan="5" class="px-4 py-8 text-center text-muted">No jobs found</td>
								</tr>
							{:else}
								{#each paginatedJobs as job (job.id)}
									<tr class="border-b transition-colors hover:bg-muted/50">
										<td class="max-w-xs truncate px-4 py-3 text-sm font-medium">
											{job.file_name}
										</td>
										<td class="px-4 py-3">
											<Badge
												variant={getStatusVariant(job.status)}
												class={getStatusClass(job.status)}
											>
												{#if job.status === 'completed'}
													<CheckCircle class="mr-1 h-3 w-3" />
												{:else if job.status === 'failed'}
													<XCircle class="mr-1 h-3 w-3" />
												{:else}
													<Clock class="mr-1 h-3 w-3" />
												{/if}
												{job.status}
											</Badge>
										</td>
										<td class="px-4 py-3 text-sm">{job.rule_name ?? '-'}</td>
										<td class="px-4 py-3 text-sm text-muted">
											{job.completed_at
												? new Date(job.completed_at).toLocaleString()
												: '-'}
										</td>
										<td class="px-4 py-3">
											<div class="flex items-center gap-1">
												{#if job.status === 'failed'}
													<Button
														variant="ghost"
														size="icon"
														onclick={() => handleRetry(job)}
														title="Retry"
													>
														<RotateCcw class="h-4 w-4" />
													</Button>
												{/if}
												<Button
													variant="ghost"
													size="icon"
													onclick={() => handleDelete(job)}
													title="Delete"
												>
													<Trash2 class="h-4 w-4" />
												</Button>
											</div>
										</td>
									</tr>
								{/each}
							{/if}
						</tbody>
					</table>
				</div>

				<!-- Pagination -->
				{#if filteredJobs.length > 0}
					<div class="mt-4 flex items-center justify-between">
						<div class="text-sm text-muted">
							Showing {currentPage * pageSize + 1}
							to {Math.min((currentPage + 1) * pageSize, filteredJobs.length)}
							of {filteredJobs.length} results
						</div>
						<div class="flex items-center gap-2">
							<Button
								variant="outline"
								size="sm"
								onclick={() => currentPage--}
								disabled={currentPage === 0}
							>
								<ChevronLeft class="h-4 w-4" />
							</Button>
							<span class="text-sm"> Page {currentPage + 1} of {totalPages || 1} </span>
							<Button
								variant="outline"
								size="sm"
								onclick={() => currentPage++}
								disabled={currentPage >= totalPages - 1}
							>
								<ChevronRight class="h-4 w-4" />
							</Button>
						</div>
					</div>
				{/if}
			{/if}
		</CardContent>
	</Card>
</div>
