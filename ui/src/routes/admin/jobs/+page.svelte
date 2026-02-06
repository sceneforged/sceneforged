<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { getJobs, retryJob, deleteJob, deleteConversion } from '$lib/api/index.js';
	import type { Job, ConversionJob } from '$lib/types.js';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import ConversionCard from '$lib/components/ConversionCard.svelte';
	import { jobsStore } from '$lib/stores/jobs.svelte.js';
	import { conversionsStore } from '$lib/stores/conversions.svelte.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import { formatDurationSecs } from '$lib/api/index.js';
	import type { AppEvent } from '$lib/types.js';
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
		ArrowUp,
		ArrowDown,
		Loader2
	} from '@lucide/svelte';

	let loading = $state(true);
	let error = $state<string | null>(null);
	let allJobs = $state<Job[]>([]);
	let globalFilter = $state('');
	let selectedJob = $state<Job | null>(null);
	let sortColumn = $state<string>('completed_at');
	let sortDesc = $state(true);
	let currentPage = $state(0);
	const pageSize = 25;
	let now = $state(Date.now());
	let tickInterval: ReturnType<typeof setInterval> | null = null;
	let unsubscribeEvents: (() => void) | null = null;

	// Filter jobs based on search
	const filteredJobs = $derived.by(() => {
		let jobs = allJobs;
		if (globalFilter) {
			const filter = globalFilter.toLowerCase();
			jobs = jobs.filter(
				(job) =>
					job.file_name?.toLowerCase().includes(filter) ||
					job.rule_name?.toLowerCase().includes(filter) ||
					job.status?.toLowerCase().includes(filter)
			);
		}

		jobs = [...jobs].sort((a, b) => {
			const aVal = a[sortColumn as keyof Job] ?? '';
			const bVal = b[sortColumn as keyof Job] ?? '';

			if (typeof aVal === 'string' && typeof bVal === 'string') {
				return sortDesc ? bVal.localeCompare(aVal) : aVal.localeCompare(bVal);
			}

			return sortDesc ? (bVal > aVal ? 1 : -1) : aVal > bVal ? 1 : -1;
		});

		return jobs;
	});

	const totalPages = $derived(Math.ceil(filteredJobs.length / pageSize));
	const paginatedJobs = $derived(
		filteredJobs.slice(currentPage * pageSize, (currentPage + 1) * pageSize)
	);

	function toggleSort(column: string) {
		if (sortColumn === column) {
			sortDesc = !sortDesc;
		} else {
			sortColumn = column;
			sortDesc = true;
		}
	}

	async function loadData() {
		loading = true;
		error = null;
		try {
			const [result] = await Promise.all([
				getJobs({ limit: 500 }),
				jobsStore.refresh(),
				conversionsStore.refresh()
			]);
			allJobs = result.jobs;
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

	async function handleCancelConversion(jobId: string) {
		try {
			await deleteConversion(jobId);
			await conversionsStore.refresh();
		} catch {
			console.error('Failed to cancel conversion job');
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
		unsubscribeEvents = eventsService.subscribe('admin', (event: AppEvent) => {
			jobsStore.handleEvent(event);
			conversionsStore.handleEvent(event);
		});
		tickInterval = setInterval(() => {
			now = Date.now();
		}, 1000);
	});

	onDestroy(() => {
		if (unsubscribeEvents) unsubscribeEvents();
		if (tickInterval) clearInterval(tickInterval);
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

	<!-- Active Processing Jobs from store -->
	{#if jobsStore.runningJobs.length > 0 || jobsStore.queuedJobs.length > 0}
		<Card class="border-blue-500/50">
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Activity class="h-5 w-5 animate-pulse text-blue-500" />
					Active Processing Jobs
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
									<p class="text-xs text-muted-foreground">
										Rule: {job.rule_name ?? 'N/A'}
									</p>
								</div>
								<Badge variant="secondary" class="bg-blue-500 text-white">
									<Activity class="mr-1 h-3 w-3 animate-pulse" />
									Running
								</Badge>
							</div>
							{#if job.progress > 0}
								<div class="space-y-1">
									<div class="flex justify-between text-xs">
										<span class="text-muted-foreground"
											>{job.current_step ?? 'Processing...'}</span
										>
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
								<p class="text-xs text-muted-foreground">
									Rule: {job.rule_name ?? 'N/A'}
								</p>
							</div>
							<Badge variant="outline">Queued</Badge>
						</div>
					{/each}
				</div>
			</CardContent>
		</Card>
	{/if}

	<!-- Active Conversions -->
	{#if conversionsStore.runningConversions.length > 0 || conversionsStore.queuedConversions.length > 0}
		<Card class="border-purple-500/50">
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Activity class="h-5 w-5 animate-pulse text-purple-500" />
					Active Conversions
					<Badge variant="secondary">
						{conversionsStore.runningConversions.length +
							conversionsStore.queuedConversions.length}
					</Badge>
				</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="space-y-4">
					{#each conversionsStore.activeConversions as cjob (cjob.id)}
						<ConversionCard
							job={cjob}
							{now}
							onCancel={handleCancelConversion}
						/>
					{/each}
				</div>
			</CardContent>
		</Card>
	{/if}

	<!-- Conversion History -->
	{#if conversionsStore.conversionHistory.length > 0}
		<Card>
			<CardHeader>
				<CardTitle>Conversion History</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="rounded-md border">
					<table class="w-full">
						<thead>
							<tr class="border-b bg-muted/50">
								<th class="px-4 py-3 text-left text-sm font-medium">Conversion</th>
								<th class="px-4 py-3 text-left text-sm font-medium">Status</th>
								<th class="px-4 py-3 text-left text-sm font-medium">Completed</th>
							</tr>
						</thead>
						<tbody>
							{#each conversionsStore.conversionHistory as job (job.id)}
								<tr class="border-b transition-colors hover:bg-muted/50">
									<td class="max-w-xs truncate px-4 py-3 text-sm font-medium">
										{job.item_name ?? job.id.slice(0, 8)}
									</td>
									<td class="px-4 py-3">
										<Badge
											variant={job.status === 'completed' ? 'default' : 'destructive'}
											class={job.status === 'completed' ? 'bg-green-500' : ''}
										>
											{#if job.status === 'completed'}
												<CheckCircle class="mr-1 h-3 w-3" />
											{:else}
												<XCircle class="mr-1 h-3 w-3" />
											{/if}
											{job.status}
										</Badge>
									</td>
									<td class="px-4 py-3 text-sm text-muted-foreground">
										{job.completed_at
											? new Date(job.completed_at).toLocaleString()
											: '-'}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
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
					<Search class="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
					<Input placeholder="Search jobs..." class="pl-8" bind:value={globalFilter} />
				</div>
			</div>
		</CardHeader>
		<CardContent>
			{#if loading && allJobs.length === 0}
				<div class="flex items-center justify-center py-12">
					<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
				</div>
			{:else}
				<div class="rounded-md border">
					<table class="w-full">
						<thead>
							<tr class="border-b bg-muted/50">
								<th
									class="cursor-pointer select-none px-4 py-3 text-left text-sm font-medium"
									onclick={() => toggleSort('file_name')}
								>
									<div class="flex items-center gap-2">
										File
										{#if sortColumn === 'file_name'}
											{#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp
													class="h-4 w-4"
												/>{/if}
										{/if}
									</div>
								</th>
								<th
									class="cursor-pointer select-none px-4 py-3 text-left text-sm font-medium"
									onclick={() => toggleSort('status')}
								>
									<div class="flex items-center gap-2">
										Status
										{#if sortColumn === 'status'}
											{#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp
													class="h-4 w-4"
												/>{/if}
										{/if}
									</div>
								</th>
								<th
									class="cursor-pointer select-none px-4 py-3 text-left text-sm font-medium"
									onclick={() => toggleSort('rule_name')}
								>
									<div class="flex items-center gap-2">
										Rule
										{#if sortColumn === 'rule_name'}
											{#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp
													class="h-4 w-4"
												/>{/if}
										{/if}
									</div>
								</th>
								<th
									class="cursor-pointer select-none px-4 py-3 text-left text-sm font-medium"
									onclick={() => toggleSort('completed_at')}
								>
									<div class="flex items-center gap-2">
										Completed
										{#if sortColumn === 'completed_at'}
											{#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp
													class="h-4 w-4"
												/>{/if}
										{/if}
									</div>
								</th>
								<th class="w-24 px-4 py-3 text-left text-sm font-medium">Actions</th>
							</tr>
						</thead>
						<tbody>
							{#if paginatedJobs.length === 0}
								<tr>
									<td
										colspan="5"
										class="px-4 py-8 text-center text-muted-foreground"
									>
										No jobs found
									</td>
								</tr>
							{:else}
								{#each paginatedJobs as job (job.id)}
									<tr
										class="cursor-pointer border-b transition-colors hover:bg-muted/50"
										onclick={() => (selectedJob = job)}
									>
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
										<td class="px-4 py-3 text-sm text-muted-foreground">
											{job.completed_at
												? new Date(job.completed_at).toLocaleString()
												: '-'}
										</td>
										<td class="px-4 py-3">
											<div
												role="group"
												class="flex items-center gap-1"
												onclick={(e: MouseEvent) => e.stopPropagation()}
											>
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
						<div class="text-sm text-muted-foreground">
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
							<span class="text-sm">
								Page {currentPage + 1} of {totalPages || 1}
							</span>
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

<!-- Job Detail Dialog -->
<Dialog.Root open={!!selectedJob} onOpenChange={(isOpen) => !isOpen && (selectedJob = null)}>
	<Dialog.Content class="max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>Job Details</Dialog.Title>
			<Dialog.Description>Detailed information about this processing job</Dialog.Description>
		</Dialog.Header>
		{#if selectedJob}
			<div class="space-y-4">
				<div class="grid grid-cols-2 gap-4 text-sm">
					<div>
						<span class="text-muted-foreground">File:</span>
						<p class="break-all font-medium">{selectedJob.file_path}</p>
					</div>
					<div>
						<span class="text-muted-foreground">Status:</span>
						<p class="font-medium">{selectedJob.status}</p>
					</div>
					<div>
						<span class="text-muted-foreground">Rule:</span>
						<p class="font-medium">{selectedJob.rule_name ?? 'N/A'}</p>
					</div>
					<div>
						<span class="text-muted-foreground">Source:</span>
						<p class="font-medium">{selectedJob.source ?? 'Unknown'}</p>
					</div>
					<div>
						<span class="text-muted-foreground">Created:</span>
						<p class="font-medium">
							{new Date(selectedJob.created_at).toLocaleString()}
						</p>
					</div>
					<div>
						<span class="text-muted-foreground">Completed:</span>
						<p class="font-medium">
							{selectedJob.completed_at
								? new Date(selectedJob.completed_at).toLocaleString()
								: '-'}
						</p>
					</div>
					<div>
						<span class="text-muted-foreground">Retries:</span>
						<p class="font-medium">
							{selectedJob.retry_count} / {selectedJob.max_retries}
						</p>
					</div>
					<div>
						<span class="text-muted-foreground">Priority:</span>
						<p class="font-medium">{selectedJob.priority}</p>
					</div>
				</div>
				{#if selectedJob.error}
					<div class="rounded-md bg-destructive/10 p-4 text-destructive">
						<span class="font-medium">Error:</span>
						<p class="mt-1 text-sm">{selectedJob.error}</p>
					</div>
				{/if}
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
