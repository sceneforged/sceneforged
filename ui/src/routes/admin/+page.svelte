<script lang="ts">
	import { onMount } from 'svelte';
	import { getDashboard } from '$lib/api/index.js';
	import type { DashboardStats } from '$lib/types.js';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { jobsStore } from '$lib/stores/jobs.svelte.js';
	import {
		Library,
		HardDrive,
		Activity,
		RefreshCw,
		FolderOpen,
		Briefcase,
		Settings
	} from '@lucide/svelte';

	let loading = $state(true);
	let error = $state<string | null>(null);
	let data = $state<DashboardStats | null>(null);

	async function loadData() {
		try {
			error = null;
			data = await getDashboard();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load dashboard';
		} finally {
			loading = false;
		}
	}

	async function handleRefresh() {
		loading = true;
		await Promise.all([loadData(), jobsStore.refresh()]);
	}

	onMount(async () => {
		await loadData();
		await jobsStore.refresh();
	});
</script>

<svelte:head>
	<title>Admin Dashboard - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold">Admin Dashboard</h1>
		<Button variant="outline" size="sm" onclick={handleRefresh} disabled={loading}>
			<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
			Refresh
		</Button>
	</div>

	{#if error}
		<div class="mb-6 rounded-md bg-destructive/10 p-4 text-destructive">
			{error}
		</div>
	{/if}

	{#if loading && !data}
		<div class="flex items-center justify-center py-20">
			<RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if data}
		<!-- Stats Cards -->
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
			<Card class="transition-shadow hover:shadow-md">
				<CardContent class="p-6">
					<div class="flex items-center gap-4">
						<div class="rounded-lg bg-primary/10 p-3">
							<Library class="h-6 w-6 text-primary" />
						</div>
						<div class="flex-1">
							<p class="text-2xl font-bold">{data.total_libraries}</p>
							<p class="text-sm text-muted-foreground">Libraries</p>
						</div>
					</div>
				</CardContent>
			</Card>

			<Card class="transition-shadow hover:shadow-md">
				<CardContent class="p-6">
					<div class="flex items-center gap-4">
						<div class="rounded-lg bg-primary/10 p-3">
							<HardDrive class="h-6 w-6 text-primary" />
						</div>
						<div class="flex-1">
							<p class="text-2xl font-bold">{data.total_items.toLocaleString()}</p>
							<p class="text-sm text-muted-foreground">Total Items</p>
						</div>
					</div>
				</CardContent>
			</Card>

			<Card class="transition-shadow hover:shadow-md">
				<CardContent class="p-6">
					<div class="flex items-center gap-4">
						<div class="rounded-lg bg-primary/10 p-3">
							<Activity class="h-6 w-6 text-primary" />
						</div>
						<div class="flex-1">
							<p class="text-2xl font-bold">{data.active_jobs}</p>
							<p class="text-sm text-muted-foreground">Active Jobs</p>
						</div>
					</div>
				</CardContent>
			</Card>

			<Card class="transition-shadow hover:shadow-md">
				<CardContent class="p-6">
					<div class="flex items-center gap-4">
						<div class="rounded-lg bg-primary/10 p-3">
							<Activity class="h-6 w-6 text-primary" />
						</div>
						<div class="flex-1">
							<p class="text-2xl font-bold">{data.completed_jobs}</p>
							<p class="text-sm text-muted-foreground">Completed Jobs</p>
						</div>
					</div>
				</CardContent>
			</Card>
		</div>

		<!-- Job queue summary -->
		<Card>
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Activity class="h-5 w-5" />
					Job Queue Summary
				</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
					<div class="text-center">
						<p class="text-2xl font-bold">{data.total_jobs}</p>
						<p class="text-sm text-muted-foreground">Total</p>
					</div>
					<div class="text-center">
						<p class="text-2xl font-bold text-blue-500">{data.active_jobs}</p>
						<p class="text-sm text-muted-foreground">Active</p>
					</div>
					<div class="text-center">
						<p class="text-2xl font-bold text-green-500">{data.completed_jobs}</p>
						<p class="text-sm text-muted-foreground">Completed</p>
					</div>
					<div class="text-center">
						<p class="text-2xl font-bold text-destructive">{data.failed_jobs}</p>
						<p class="text-sm text-muted-foreground">Failed</p>
					</div>
				</div>
			</CardContent>
		</Card>

		<!-- Tools Status -->
		{#if data.tools.length > 0}
			<Card>
				<CardHeader>
					<CardTitle>External Tools</CardTitle>
				</CardHeader>
				<CardContent>
					<div class="flex flex-wrap gap-2">
						{#each data.tools as tool}
							<Badge
								variant={tool.available ? 'default' : 'destructive'}
								class={tool.available ? 'bg-green-500' : ''}
							>
								{tool.name}
								{#if tool.version}
									({tool.version})
								{/if}
							</Badge>
						{/each}
					</div>
				</CardContent>
			</Card>
		{/if}

		<!-- Quick Links -->
		<Card>
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<FolderOpen class="h-5 w-5" />
					Quick Links
				</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
					<a href="/admin/libraries" class="block">
						<Button variant="outline" class="h-auto w-full flex-col gap-2 py-4">
							<Library class="h-6 w-6" />
							<span>Libraries</span>
						</Button>
					</a>
					<a href="/admin/jobs" class="block">
						<Button variant="outline" class="h-auto w-full flex-col gap-2 py-4">
							<Briefcase class="h-6 w-6" />
							<span>Jobs</span>
						</Button>
					</a>
					<a href="/rules" class="block">
						<Button variant="outline" class="h-auto w-full flex-col gap-2 py-4">
							<Settings class="h-6 w-6" />
							<span>Rules</span>
						</Button>
					</a>
					<a href="/settings" class="block">
						<Button variant="outline" class="h-auto w-full flex-col gap-2 py-4">
							<Settings class="h-6 w-6" />
							<span>Settings</span>
						</Button>
					</a>
				</div>
			</CardContent>
		</Card>
	{/if}
</div>
