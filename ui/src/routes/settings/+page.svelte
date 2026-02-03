<script lang="ts">
	import { onMount } from 'svelte';
	import { getTools } from '$lib/api/index.js';
	import type { ToolInfo } from '$lib/types.js';
	import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card/index.js';
	import Badge from '$lib/components/ui/badge/Badge.svelte';
	import Button from '$lib/components/ui/button/Button.svelte';
	import {
		Settings,
		RefreshCw,
		CheckCircle,
		XCircle,
		AlertTriangle,
		Wrench,
		Film,
		HardDrive,
		FolderOpen
	} from 'lucide-svelte';

	let tools = $state<ToolInfo[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function loadData() {
		loading = true;
		error = null;
		try {
			tools = await getTools();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load settings';
		} finally {
			loading = false;
		}
	}

	function getToolIcon(name: string) {
		switch (name.toLowerCase()) {
			case 'ffmpeg':
			case 'ffprobe':
				return Film;
			case 'mediainfo':
				return HardDrive;
			case 'mkvmerge':
				return FolderOpen;
			default:
				return Wrench;
		}
	}

	onMount(() => {
		loadData();
	});
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold">Settings</h1>
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

	<!-- Server Status -->
	<Card>
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<Settings class="h-5 w-5" />
				Server Configuration
			</CardTitle>
			<CardDescription>Server status and configuration</CardDescription>
		</CardHeader>
		<CardContent>
			<div class="flex items-center gap-3">
				<div class="flex h-10 w-10 items-center justify-center rounded-full bg-green-500/10">
					<CheckCircle class="h-5 w-5 text-green-500" />
				</div>
				<div>
					<p class="font-medium">Running</p>
					<p class="text-sm text-muted">Server is operational</p>
				</div>
			</div>
		</CardContent>
	</Card>

	<!-- External Tools -->
	<Card>
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<Wrench class="h-5 w-5" />
				External Tools
			</CardTitle>
			<CardDescription>Required tools for media processing</CardDescription>
		</CardHeader>
		<CardContent>
			{#if tools.length === 0 && !loading}
				<p class="py-4 text-center text-muted">No tools detected</p>
			{:else}
				<div class="space-y-3">
					{#each tools as tool}
						{@const Icon = getToolIcon(tool.name)}
						<div class="flex items-center justify-between rounded-lg border p-3">
							<div class="flex items-center gap-3">
								<div
									class="flex h-10 w-10 items-center justify-center rounded-lg {tool.available
										? 'bg-green-500/10'
										: 'bg-destructive/10'}"
								>
									<Icon
										class="h-5 w-5 {tool.available ? 'text-green-500' : 'text-destructive'}"
									/>
								</div>
								<div>
									<p class="font-medium">{tool.name}</p>
									{#if tool.version}
										<p class="text-xs text-muted">{tool.version}</p>
									{/if}
								</div>
							</div>
							<div class="flex items-center gap-2">
								{#if tool.path}
									<code class="hidden rounded bg-muted px-2 py-1 text-xs md:block">
										{tool.path}
									</code>
								{/if}
								{#if tool.available}
									<Badge variant="default" class="bg-green-500">
										<CheckCircle class="mr-1 h-3 w-3" />
										Installed
									</Badge>
								{:else}
									<Badge variant="destructive">
										<XCircle class="mr-1 h-3 w-3" />
										Missing
									</Badge>
								{/if}
							</div>
						</div>
					{/each}
				</div>

				{@const missing = tools.filter((t) => !t.available)}
				{#if missing.length > 0}
					<div class="mt-4 rounded-lg bg-amber-500/10 p-4 text-amber-700 dark:text-amber-300">
						<div class="flex items-center gap-2">
							<AlertTriangle class="h-4 w-4" />
							<span class="font-medium">Missing Tools</span>
						</div>
						<p class="mt-1 text-sm">
							Install {missing.map((t) => t.name).join(', ')} to enable all features.
						</p>
					</div>
				{/if}
			{/if}
		</CardContent>
	</Card>
</div>
