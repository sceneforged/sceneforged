<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { getLibraries, createLibrary, deleteLibrary, scanLibrary } from '$lib/api/index.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import type { Library } from '$lib/types.js';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import {
		Library as LibraryIcon,
		Plus,
		Trash2,
		RefreshCw,
		Film,
		Tv,
		Music,
		FolderOpen,
		Loader2,
		ChevronRight
	} from '@lucide/svelte';

	let libraries = $state<Library[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// New library form
	let showForm = $state(false);
	let formName = $state('');
	let formType = $state('movies');
	let formPath = $state('');
	let creating = $state(false);

	// Scan state
	let scanningLibrary = $state<string | null>(null);
	let deletingLibrary = $state<string | null>(null);

	async function loadLibraries() {
		loading = true;
		error = null;
		try {
			libraries = await getLibraries();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load libraries';
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!formName.trim() || !formPath.trim()) {
			error = 'Name and path are required';
			return;
		}

		creating = true;
		error = null;
		try {
			await createLibrary({
				name: formName.trim(),
				media_type: formType,
				paths: [formPath.trim()]
			});
			await loadLibraries();
			showForm = false;
			formName = '';
			formType = 'movies';
			formPath = '';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to create library';
		} finally {
			creating = false;
		}
	}

	async function handleDelete(lib: Library) {
		if (!confirm(`Delete library "${lib.name}"? This will remove all items from the database.`))
			return;

		deletingLibrary = lib.id;
		try {
			await deleteLibrary(lib.id);
			libraries = libraries.filter((l) => l.id !== lib.id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete library';
		} finally {
			deletingLibrary = null;
		}
	}

	async function handleScan(lib: Library) {
		scanningLibrary = lib.id;
		try {
			await scanLibrary(lib.id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to start scan';
			scanningLibrary = null;
		}
	}

	function getMediaTypeIcon(type: string) {
		switch (type) {
			case 'movies':
				return Film;
			case 'tvshows':
				return Tv;
			case 'music':
				return Music;
			default:
				return FolderOpen;
		}
	}

	let unsubscribe: (() => void) | null = null;

	onMount(() => {
		loadLibraries();
		unsubscribe = eventsService.subscribe('all', (event) => {
			const { payload } = event;
			if (payload.type === 'library_scan_complete') {
				if (scanningLibrary === payload.library_id) {
					scanningLibrary = null;
				}
				loadLibraries();
			}
		});
	});

	onDestroy(() => {
		unsubscribe?.();
	});
</script>

<svelte:head>
	<title>Libraries - Admin - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<LibraryIcon class="h-8 w-8 text-primary" />
			<h1 class="text-2xl font-bold">Libraries</h1>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" onclick={loadLibraries} disabled={loading}>
				<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
				Refresh
			</Button>
			<Button size="sm" onclick={() => (showForm = !showForm)}>
				<Plus class="mr-2 h-4 w-4" />
				Add Library
			</Button>
		</div>
	</div>

	{#if error}
		<div class="rounded-md bg-destructive/10 p-4 text-destructive">
			{error}
		</div>
	{/if}

	<!-- New Library Form -->
	{#if showForm}
		<Card>
			<CardHeader>
				<CardTitle>New Library</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="space-y-4">
					<div>
						<label for="lib-name" class="text-sm font-medium">Name</label>
						<Input id="lib-name" bind:value={formName} placeholder="My Movies" />
					</div>

					<div>
						<span class="text-sm font-medium">Type</span>
						<div class="mt-2 flex gap-4">
							<label class="flex items-center gap-2">
								<input type="radio" bind:group={formType} value="movies" />
								<Film class="h-4 w-4" />
								Movies
							</label>
							<label class="flex items-center gap-2">
								<input type="radio" bind:group={formType} value="tvshows" />
								<Tv class="h-4 w-4" />
								TV Shows
							</label>
							<label class="flex items-center gap-2">
								<input type="radio" bind:group={formType} value="music" />
								<Music class="h-4 w-4" />
								Music
							</label>
						</div>
					</div>

					<div>
						<label for="lib-path" class="text-sm font-medium">Path</label>
						<Input id="lib-path" bind:value={formPath} placeholder="/media/movies" />
					</div>

					<div class="flex gap-2">
						<Button onclick={handleCreate} disabled={creating}>
							{#if creating}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							{/if}
							Create Library
						</Button>
						<Button variant="outline" onclick={() => (showForm = false)}>Cancel</Button>
					</div>
				</div>
			</CardContent>
		</Card>
	{/if}

	<!-- Libraries List -->
	{#if loading && libraries.length === 0}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if libraries.length === 0}
		<Card>
			<CardContent class="py-12 text-center">
				<LibraryIcon class="mx-auto mb-4 h-16 w-16 text-muted-foreground/30" />
				<h2 class="text-lg font-medium text-muted-foreground">No libraries</h2>
				<p class="mt-1 text-sm text-muted-foreground">Add a library to start organizing your media.</p>
			</CardContent>
		</Card>
	{:else}
		<div class="grid gap-4">
			{#each libraries as lib (lib.id)}
				{@const Icon = getMediaTypeIcon(lib.media_type)}
				<Card class="transition-colors hover:border-primary/50">
					<CardContent class="p-4">
						<div class="flex items-start justify-between">
							<a
								href="/admin/libraries/{lib.id}"
								class="group flex min-w-0 flex-1 items-start gap-3"
							>
								<div
									class="rounded-lg bg-muted p-2 transition-colors group-hover:bg-primary/10"
								>
									<Icon class="h-6 w-6 text-primary" />
								</div>
								<div class="min-w-0 flex-1">
									<div class="flex items-center gap-2">
										<h3
											class="font-medium transition-colors group-hover:text-primary"
										>
											{lib.name}
										</h3>
										<ChevronRight
											class="h-4 w-4 text-muted-foreground transition-colors group-hover:text-primary"
										/>
									</div>
									<div class="mt-1 flex items-center gap-2">
										<Badge variant="outline">{lib.media_type}</Badge>
									</div>
									<div class="mt-2 text-sm text-muted-foreground">
										{#each lib.paths as path}
											<div class="truncate font-mono text-xs">{path}</div>
										{/each}
									</div>
								</div>
							</a>
							<div class="ml-4 flex items-center gap-2">
								<Button
									variant="outline"
									size="sm"
									onclick={() => handleScan(lib)}
									disabled={scanningLibrary === lib.id}
								>
									{#if scanningLibrary === lib.id}
										<Loader2 class="mr-2 h-4 w-4 animate-spin" />
										Scanning
									{:else}
										Scan
									{/if}
								</Button>
								<Button
									variant="destructive"
									size="sm"
									onclick={() => handleDelete(lib)}
									disabled={deletingLibrary === lib.id}
								>
									<Trash2 class="h-4 w-4" />
								</Button>
							</div>
						</div>
					</CardContent>
				</Card>
			{/each}
		</div>
	{/if}
</div>
