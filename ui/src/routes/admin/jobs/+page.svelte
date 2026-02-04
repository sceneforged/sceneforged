<script lang="ts">
	import { onMount } from 'svelte';
	import { getJobs, retryJob, deleteJob, getConfigRules, updateConfigRules } from '$lib/api/index.js';
	import type { Job, Rule } from '$lib/types.js';
	import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '$lib/components/ui/collapsible/index.js';
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
		Loader2,
		BookOpen,
		ChevronDown,
		Pencil,
		Plus,
		Save,
		X as XIcon
	} from '@lucide/svelte';

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

	// --- Rules section ---
	let rules = $state<Rule[]>([]);
	let rulesLoading = $state(true);
	let rulesError = $state<string | null>(null);
	let rulesOpen = $state(false);

	// Rules editor
	let rulesEditorOpen = $state(false);
	let rulesEditingIndex = $state<number | null>(null);
	let rulesEditorName = $state('');
	let rulesEditorEnabled = $state(true);
	let rulesEditorPriority = $state(0);

	async function loadRules() {
		rulesLoading = true;
		rulesError = null;
		try {
			rules = await getConfigRules();
		} catch (e) {
			rulesError = e instanceof Error ? e.message : 'Failed to load rules';
		} finally {
			rulesLoading = false;
		}
	}

	async function handleToggleRuleEnabled(index: number) {
		const updated = rules.map((r, i) => (i === index ? { ...r, enabled: !r.enabled } : r));
		try {
			rules = await updateConfigRules(updated);
		} catch (e) {
			rulesError = e instanceof Error ? e.message : 'Failed to update rule';
		}
	}

	async function handleDeleteRule(index: number) {
		const ruleName = rules[index]?.name;
		if (!confirm(`Delete rule "${ruleName}"?`)) return;
		const updated = rules.filter((_, i) => i !== index);
		try {
			rules = await updateConfigRules(updated);
		} catch (e) {
			rulesError = e instanceof Error ? e.message : 'Failed to delete rule';
		}
	}

	function openRulesEditor(index: number | null = null) {
		rulesEditingIndex = index;
		if (index !== null && rules[index]) {
			const rule = rules[index];
			rulesEditorName = rule.name;
			rulesEditorEnabled = rule.enabled;
			rulesEditorPriority = rule.priority;
		} else {
			rulesEditorName = '';
			rulesEditorEnabled = true;
			rulesEditorPriority = rules.length > 0 ? Math.max(...rules.map((r) => r.priority)) + 1 : 1;
		}
		rulesEditorOpen = true;
	}

	async function handleSaveRule() {
		if (!rulesEditorName.trim()) return;
		const updatedRule: Rule = {
			id: rulesEditingIndex !== null && rules[rulesEditingIndex] ? rules[rulesEditingIndex].id : '',
			name: rulesEditorName.trim(),
			enabled: rulesEditorEnabled,
			priority: rulesEditorPriority,
			expr: rulesEditingIndex !== null && rules[rulesEditingIndex] ? rules[rulesEditingIndex].expr : {},
			actions: rulesEditingIndex !== null && rules[rulesEditingIndex] ? rules[rulesEditingIndex].actions : []
		};
		let updated: Rule[];
		if (rulesEditingIndex !== null) {
			updated = rules.map((r, i) => (i === rulesEditingIndex ? updatedRule : r));
		} else {
			updated = [...rules, updatedRule];
		}
		try {
			rules = await updateConfigRules(updated);
			rulesEditorOpen = false;
		} catch (e) {
			rulesError = e instanceof Error ? e.message : 'Failed to save rule';
		}
	}

	function formatConditions(rule: Rule): string[] {
		if (!rule.expr) return [];
		if (typeof rule.expr === 'object') {
			const entries = Object.entries(rule.expr as Record<string, unknown>);
			return entries.map(([key, value]) => {
				if (typeof value === 'object' && value !== null) {
					return `${key}: ${JSON.stringify(value)}`;
				}
				return `${key}: ${value}`;
			});
		}
		return [String(rule.expr)];
	}

	function formatAction(action: Record<string, unknown>): string {
		const type = action.type as string;
		if (!type) return 'Unknown action';
		return type.replace(/_/g, ' ').replace(/\b\w/g, (l) => l.toUpperCase());
	}

	onMount(() => {
		loadData();
		loadRules();
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
									<p class="text-xs text-muted-foreground">Rule: {job.rule_name ?? 'N/A'}</p>
								</div>
								<Badge variant="secondary" class="bg-blue-500 text-white">
									<Activity class="mr-1 h-3 w-3 animate-pulse" />
									Running
								</Badge>
							</div>
							{#if job.progress > 0}
								<div class="space-y-1">
									<div class="flex justify-between text-xs">
										<span class="text-muted-foreground">{job.current_step ?? 'Processing...'}</span>
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
								<p class="text-xs text-muted-foreground">Rule: {job.rule_name ?? 'N/A'}</p>
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
									<td colspan="5" class="px-4 py-8 text-center text-muted-foreground">No jobs found</td>
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
										<td class="px-4 py-3 text-sm text-muted-foreground">
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

	<!-- Rules Section -->
	<Collapsible bind:open={rulesOpen}>
		<Card>
			<CardHeader>
				<CollapsibleTrigger class="flex w-full items-center justify-between">
					<CardTitle class="flex items-center gap-2">
						<BookOpen class="h-5 w-5" />
						Processing Rules
						{#if rules.length > 0}
							<Badge variant="secondary">{rules.length}</Badge>
						{/if}
					</CardTitle>
					<ChevronDown
						class="h-5 w-5 text-muted-foreground transition-transform {rulesOpen ? 'rotate-180' : ''}"
					/>
				</CollapsibleTrigger>
			</CardHeader>
			<CollapsibleContent>
				<CardContent>
				<div class="space-y-4">
					<div class="flex items-center justify-between">
						<p class="text-sm text-muted-foreground">
							Rules define how media files are automatically processed.
						</p>
						<div class="flex items-center gap-2">
							<Button variant="default" size="sm" onclick={() => openRulesEditor()}>
								<Plus class="mr-2 h-4 w-4" />
								New Rule
							</Button>
							<Button variant="outline" size="sm" onclick={loadRules} disabled={rulesLoading}>
								<RefreshCw class="mr-2 h-4 w-4 {rulesLoading ? 'animate-spin' : ''}" />
								Refresh
							</Button>
						</div>
					</div>

					{#if rulesError}
						<div class="rounded-md bg-destructive/10 p-4 text-destructive">
							{rulesError}
						</div>
					{/if}

					<!-- Rules editor -->
					{#if rulesEditorOpen}
						<Card class="border-primary">
							<CardHeader>
								<CardTitle>{rulesEditingIndex !== null ? 'Edit Rule' : 'New Rule'}</CardTitle>
							</CardHeader>
							<CardContent>
								<div class="space-y-4">
									<div class="space-y-2">
										<label for="rule-name" class="text-sm font-medium">Name</label>
										<Input id="rule-name" bind:value={rulesEditorName} placeholder="Rule name" />
									</div>
									<div class="space-y-2">
										<label for="rule-priority" class="text-sm font-medium">Priority</label>
										<Input id="rule-priority" type="number" bind:value={rulesEditorPriority} />
									</div>
									<label class="flex items-center gap-2">
										<input type="checkbox" bind:checked={rulesEditorEnabled} class="h-4 w-4" />
										<span class="text-sm font-medium">Enabled</span>
									</label>
									<div class="flex gap-2">
										<Button onclick={handleSaveRule}>
											<Save class="mr-2 h-4 w-4" />
											Save
										</Button>
										<Button variant="outline" onclick={() => (rulesEditorOpen = false)}>
											<XIcon class="mr-2 h-4 w-4" />
											Cancel
										</Button>
									</div>
								</div>
							</CardContent>
						</Card>
					{/if}

					{#if rulesLoading}
						<div class="py-8 text-center text-muted-foreground">
							<RefreshCw class="mx-auto mb-2 h-6 w-6 animate-spin" />
							<p class="text-sm">Loading rules...</p>
						</div>
					{:else if rules.length === 0}
						<div class="py-8 text-center text-muted-foreground">
							<BookOpen class="mx-auto mb-2 h-8 w-8 opacity-50" />
							<p class="text-sm">No rules configured</p>
						</div>
					{:else}
						<div class="space-y-3">
							{#each [...rules].sort((a, b) => b.priority - a.priority) as rule, index}
								<div
									class="flex items-center justify-between rounded-lg border p-3 {rule.enabled ? '' : 'opacity-60'}"
								>
									<div class="flex items-center gap-3">
										<div
											class="flex h-7 w-7 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary"
										>
											{index + 1}
										</div>
										<div>
											<div class="flex items-center gap-2">
												<span class="text-sm font-medium">{rule.name}</span>
												<button onclick={() => handleToggleRuleEnabled(index)}>
													{#if rule.enabled}
														<Badge variant="default" class="cursor-pointer bg-green-500 text-xs">
															Active
														</Badge>
													{:else}
														<Badge variant="secondary" class="cursor-pointer text-xs">
															Disabled
														</Badge>
													{/if}
												</button>
											</div>
											<span class="text-xs text-muted-foreground">Priority: {rule.priority}</span>
											{#if formatConditions(rule).length > 0}
												<span class="ml-2 text-xs text-muted-foreground">
													| {formatConditions(rule).join(', ')}
												</span>
											{/if}
										</div>
									</div>
									<div class="flex items-center gap-1">
										<Button
											variant="ghost"
											size="icon"
											onclick={() => openRulesEditor(index)}
											title="Edit"
										>
											<Pencil class="h-4 w-4" />
										</Button>
										<Button
											variant="ghost"
											size="icon"
											onclick={() => handleDeleteRule(index)}
											title="Delete"
										>
											<Trash2 class="h-4 w-4" />
										</Button>
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</CardContent>
			</CollapsibleContent>
		</Card>
	</Collapsible>
</div>
