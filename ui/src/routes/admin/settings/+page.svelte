<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getTools,
		getLibraries,
		createLibrary,
		deleteLibrary,
		scanLibrary,
		getConfigArrs,
		createArr,
		updateArr,
		deleteArr,
		testArr,
		getConfigJellyfins,
		createJellyfin,
		updateJellyfin,
		deleteJellyfin,
		getConversionConfig,
		updateConversionConfig,
		reloadConfig
	} from '$lib/api/index.js';
	import type {
		ToolInfo,
		Library,
		ArrConfig,
		JellyfinConfig,
		ConversionConfig
	} from '$lib/types.js';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle,
		CardDescription
	} from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import {
		Settings,
		RefreshCw,
		CheckCircle,
		XCircle,
		AlertTriangle,
		Wrench,
		Film,
		HardDrive,
		FolderOpen,
		Plus,
		Trash2,
		Pencil,
		Loader2,
		Tv,
		Library as LibraryIcon,
		AlertCircle,
		Music
	} from '@lucide/svelte';

	// State
	let tools = $state<ToolInfo[]>([]);
	let libraries = $state<Library[]>([]);
	let arrs = $state<ArrConfig[]>([]);
	let jellyfins = $state<JellyfinConfig[]>([]);
	let conversionSettings = $state<ConversionConfig>({
		auto_convert_on_scan: false,
		auto_convert_dv_p7_to_p8: false,
		video_crf: 15,
		video_preset: 'slow',
		audio_bitrate: '256k',
		adaptive_crf: true
	});

	let loading = $state(true);
	let reloading = $state(false);
	let error = $state<string | null>(null);
	let toolsError = $state<string | null>(null);

	// Library state
	let scanningLibrary = $state<string | null>(null);
	let deletingLibrary = $state<string | null>(null);
	let libraryEditorOpen = $state(false);
	let libraryLoading = $state(false);
	let libraryError = $state<string | null>(null);
	let libraryForm = $state({ name: '', media_type: 'movies', paths: [''] });

	// Arr state
	let arrEditorOpen = $state(false);
	let editingArr = $state<ArrConfig | null>(null);
	let arrLoading = $state(false);
	let arrError = $state<string | null>(null);
	let testingArr = $state<string | null>(null);
	let deletingArr = $state<string | null>(null);
	let arrForm = $state({
		name: '',
		type: 'radarr',
		url: '',
		api_key: '',
		enabled: true,
		auto_rescan: true,
		auto_rename: false
	});

	// Jellyfin state
	let jellyfinEditorOpen = $state(false);
	let editingJellyfin = $state<JellyfinConfig | null>(null);
	let jellyfinLoading = $state(false);
	let jellyfinError = $state<string | null>(null);
	let deletingJellyfin = $state<string | null>(null);
	let jellyfinForm = $state({
		name: '',
		url: '',
		api_key: '',
		enabled: true
	});

	async function loadData() {
		loading = true;
		error = null;
		toolsError = null;
		try {
			const [toolsResult, librariesData, arrsData, jellyfinsData, convData] = await Promise.all([
				getTools()
					.then((data) => ({ data, error: null }))
					.catch((e) => ({ data: [] as ToolInfo[], error: e instanceof Error ? e.message : 'Failed to load tools' })),
				getLibraries().catch(() => [] as Library[]),
				getConfigArrs().catch(() => [] as ArrConfig[]),
				getConfigJellyfins().catch(() => [] as JellyfinConfig[]),
				getConversionConfig().catch(
					() =>
						({
							auto_convert_on_scan: false,
							auto_convert_dv_p7_to_p8: false,
							video_crf: 15,
							video_preset: 'slow',
							audio_bitrate: '256k',
							adaptive_crf: true
						}) as ConversionConfig
				)
			]);
			tools = toolsResult.data;
			toolsError = toolsResult.error;
			libraries = librariesData;
			arrs = arrsData;
			jellyfins = jellyfinsData;
			conversionSettings = convData;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load settings';
		} finally {
			loading = false;
		}
	}

	async function handleReloadConfig() {
		reloading = true;
		try {
			await reloadConfig();
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to reload config';
		} finally {
			reloading = false;
		}
	}

	// --- Library handlers ---

	function getMediaTypeIcon(mediaType: string) {
		switch (mediaType) {
			case 'movies':
				return Film;
			case 'tvshows':
				return Tv;
			case 'music':
				return Music;
			default:
				return LibraryIcon;
		}
	}

	function openLibraryEditor() {
		libraryForm = { name: '', media_type: 'movies', paths: [''] };
		libraryError = null;
		libraryEditorOpen = true;
	}

	async function handleSaveLibrary() {
		libraryLoading = true;
		libraryError = null;
		try {
			const paths = libraryForm.paths.filter((p) => p.trim() !== '');
			if (!libraryForm.name.trim()) {
				libraryError = 'Name is required';
				return;
			}
			if (paths.length === 0) {
				libraryError = 'At least one path is required';
				return;
			}
			await createLibrary({
				name: libraryForm.name,
				media_type: libraryForm.media_type,
				paths
			});
			libraryEditorOpen = false;
			libraries = await getLibraries();
		} catch (e) {
			libraryError = e instanceof Error ? e.message : 'Failed to create library';
		} finally {
			libraryLoading = false;
		}
	}

	async function handleScanLibrary(id: string) {
		scanningLibrary = id;
		try {
			await scanLibrary(id);
		} finally {
			scanningLibrary = null;
		}
	}

	async function handleDeleteLibrary(id: string, name: string) {
		if (!confirm(`Delete library "${name}"? This cannot be undone.`)) return;
		deletingLibrary = id;
		try {
			await deleteLibrary(id);
			libraries = libraries.filter((l) => l.id !== id);
		} finally {
			deletingLibrary = null;
		}
	}

	// --- Arr handlers ---

	function openArrEditor(arr?: ArrConfig) {
		if (arr) {
			editingArr = arr;
			arrForm = {
				name: arr.name,
				type: arr.type,
				url: arr.url,
				api_key: '',
				enabled: arr.enabled,
				auto_rescan: arr.auto_rescan,
				auto_rename: arr.auto_rename
			};
		} else {
			editingArr = null;
			arrForm = {
				name: '',
				type: 'radarr',
				url: '',
				api_key: '',
				enabled: true,
				auto_rescan: true,
				auto_rename: false
			};
		}
		arrError = null;
		arrEditorOpen = true;
	}

	async function handleSaveArr() {
		arrLoading = true;
		arrError = null;
		try {
			if (!arrForm.name.trim()) {
				arrError = 'Name is required';
				return;
			}
			if (!arrForm.url.trim()) {
				arrError = 'URL is required';
				return;
			}

			const data: ArrConfig = {
				name: arrForm.name,
				type: arrForm.type,
				url: arrForm.url,
				api_key: arrForm.api_key || (editingArr?.api_key ?? ''),
				enabled: arrForm.enabled,
				auto_rescan: arrForm.auto_rescan,
				auto_rename: arrForm.auto_rename
			};

			if (editingArr) {
				await updateArr(editingArr.name, data);
			} else {
				if (!arrForm.api_key.trim()) {
					arrError = 'API key is required';
					return;
				}
				await createArr(data);
			}
			arrEditorOpen = false;
			arrs = await getConfigArrs();
		} catch (e) {
			arrError = e instanceof Error ? e.message : 'Failed to save arr';
		} finally {
			arrLoading = false;
		}
	}

	async function handleTestArr(name: string) {
		testingArr = name;
		try {
			const result = await testArr(name);
			alert(result.message);
		} catch (e) {
			alert(e instanceof Error ? e.message : 'Test failed');
		} finally {
			testingArr = null;
		}
	}

	async function handleDeleteArr(name: string) {
		if (!confirm(`Delete arr "${name}"?`)) return;
		deletingArr = name;
		try {
			await deleteArr(name);
			arrs = arrs.filter((a) => a.name !== name);
		} finally {
			deletingArr = null;
		}
	}

	// --- Jellyfin handlers ---

	function openJellyfinEditor(jf?: JellyfinConfig) {
		if (jf) {
			editingJellyfin = jf;
			jellyfinForm = {
				name: jf.name,
				url: jf.url,
				api_key: '',
				enabled: jf.enabled
			};
		} else {
			editingJellyfin = null;
			jellyfinForm = { name: '', url: '', api_key: '', enabled: true };
		}
		jellyfinError = null;
		jellyfinEditorOpen = true;
	}

	async function handleSaveJellyfin() {
		jellyfinLoading = true;
		jellyfinError = null;
		try {
			if (!jellyfinForm.name.trim()) {
				jellyfinError = 'Name is required';
				return;
			}
			if (!jellyfinForm.url.trim()) {
				jellyfinError = 'URL is required';
				return;
			}

			const data: JellyfinConfig = {
				name: jellyfinForm.name,
				url: jellyfinForm.url,
				api_key: jellyfinForm.api_key || (editingJellyfin?.api_key ?? ''),
				enabled: jellyfinForm.enabled
			};

			if (editingJellyfin) {
				await updateJellyfin(editingJellyfin.name, data);
			} else {
				if (!jellyfinForm.api_key.trim()) {
					jellyfinError = 'API key is required';
					return;
				}
				await createJellyfin(data);
			}
			jellyfinEditorOpen = false;
			jellyfins = await getConfigJellyfins();
		} catch (e) {
			jellyfinError = e instanceof Error ? e.message : 'Failed to save jellyfin';
		} finally {
			jellyfinLoading = false;
		}
	}

	async function handleDeleteJellyfin(name: string) {
		if (!confirm(`Delete Jellyfin "${name}"?`)) return;
		deletingJellyfin = name;
		try {
			await deleteJellyfin(name);
			jellyfins = jellyfins.filter((j) => j.name !== name);
		} finally {
			deletingJellyfin = null;
		}
	}

	// --- Conversion handlers ---

	async function handleConversionSettingChange() {
		try {
			await updateConversionConfig(conversionSettings);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save conversion settings';
		}
	}

	// --- Tool helpers ---

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

<svelte:head>
	<title>Server Settings - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold">Server Settings</h1>
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" onclick={handleReloadConfig} disabled={reloading}>
				{#if reloading}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{:else}
					<RefreshCw class="mr-2 h-4 w-4" />
				{/if}
				Reload Config
			</Button>
			<Button variant="outline" size="sm" onclick={loadData} disabled={loading}>
				<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
				Refresh
			</Button>
		</div>
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
					<p class="text-sm text-muted-foreground">Server is operational</p>
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
			{#if toolsError}
				<div class="flex items-center gap-2 rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-destructive">
					<AlertCircle class="h-4 w-4" />
					<span>{toolsError}</span>
				</div>
			{:else if tools.length === 0 && !loading}
				<p class="py-4 text-center text-muted-foreground">No tools detected</p>
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
										<p class="text-xs text-muted-foreground">{tool.version}</p>
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

	<!-- Libraries -->
	<Card>
		<CardHeader>
			<div class="flex items-center justify-between">
				<div>
					<CardTitle class="flex items-center gap-2">
						<LibraryIcon class="h-5 w-5" />
						Libraries
					</CardTitle>
					<CardDescription>Media library paths for scanning</CardDescription>
				</div>
				<Button size="sm" onclick={openLibraryEditor}>
					<Plus class="mr-2 h-4 w-4" />
					Add
				</Button>
			</div>
		</CardHeader>
		<CardContent>
			{#if libraries.length === 0}
				<div class="py-8 text-center text-muted-foreground">
					<LibraryIcon class="mx-auto mb-2 h-12 w-12 opacity-50" />
					<p>No libraries configured</p>
					<p class="mt-1 text-sm">Click "Add" to create your first library</p>
				</div>
			{:else}
				<div class="space-y-3">
					{#each libraries as library}
						{@const TypeIcon = getMediaTypeIcon(library.media_type)}
						<div class="flex items-center justify-between rounded-lg border p-3">
							<div class="flex items-center gap-3">
								<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
									<TypeIcon class="h-5 w-5 text-primary" />
								</div>
								<div>
									<p class="font-medium">{library.name}</p>
									<p class="text-xs text-muted-foreground">
										{library.media_type} &middot; {library.paths.length} path{library.paths
											.length !== 1
											? 's'
											: ''}
									</p>
								</div>
							</div>
							<div class="flex items-center gap-2">
								<Button
									variant="outline"
									size="sm"
									onclick={() => handleScanLibrary(library.id)}
									disabled={scanningLibrary === library.id}
								>
									{#if scanningLibrary === library.id}
										<RefreshCw class="h-4 w-4 animate-spin" />
									{:else}
										Scan
									{/if}
								</Button>
								<Button
									variant="ghost"
									size="icon"
									onclick={() => handleDeleteLibrary(library.id, library.name)}
									disabled={deletingLibrary === library.id}
								>
									<Trash2
										class="h-4 w-4 {deletingLibrary === library.id ? 'animate-pulse' : ''}"
									/>
								</Button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</CardContent>
	</Card>

	<!-- *Arr Integrations -->
	<Card>
		<CardHeader>
			<div class="flex items-center justify-between">
				<div>
					<CardTitle class="flex items-center gap-2">
						<Tv class="h-5 w-5" />
						*Arr Integrations
					</CardTitle>
					<CardDescription>Radarr and Sonarr connections</CardDescription>
				</div>
				<Button size="sm" onclick={() => openArrEditor()}>
					<Plus class="mr-2 h-4 w-4" />
					Add
				</Button>
			</div>
		</CardHeader>
		<CardContent>
			{#if arrs.length === 0}
				<div class="py-8 text-center text-muted-foreground">
					<Tv class="mx-auto mb-2 h-12 w-12 opacity-50" />
					<p>No *arr integrations configured</p>
					<p class="mt-1 text-sm">Click "Add" to create one</p>
				</div>
			{:else}
				<div class="space-y-3">
					{#each arrs as arr}
						<div class="flex items-center justify-between rounded-lg border p-3">
							<div class="flex items-center gap-3">
								<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
									{#if arr.type === 'radarr'}
										<Film class="h-5 w-5 text-primary" />
									{:else}
										<Tv class="h-5 w-5 text-primary" />
									{/if}
								</div>
								<div>
									<p class="font-medium">{arr.name}</p>
									<p class="text-xs text-muted-foreground">{arr.url}</p>
								</div>
							</div>
							<div class="flex items-center gap-2">
								{#if arr.enabled}
									<Button
										variant="outline"
										size="sm"
										onclick={() => handleTestArr(arr.name)}
										disabled={testingArr === arr.name}
									>
										{#if testingArr === arr.name}
											<RefreshCw class="h-4 w-4 animate-spin" />
										{:else}
											Test
										{/if}
									</Button>
								{:else}
									<Badge variant="secondary">Disabled</Badge>
								{/if}
								<Button variant="ghost" size="icon" onclick={() => openArrEditor(arr)}>
									<Pencil class="h-4 w-4" />
								</Button>
								<Button
									variant="ghost"
									size="icon"
									onclick={() => handleDeleteArr(arr.name)}
									disabled={deletingArr === arr.name}
								>
									<Trash2
										class="h-4 w-4 {deletingArr === arr.name ? 'animate-pulse' : ''}"
									/>
								</Button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</CardContent>
	</Card>

	<!-- Jellyfin Integrations -->
	<Card>
		<CardHeader>
			<div class="flex items-center justify-between">
				<div>
					<CardTitle class="flex items-center gap-2">
						<Film class="h-5 w-5" />
						Jellyfin Integrations
					</CardTitle>
					<CardDescription>Media server notifications</CardDescription>
				</div>
				<Button size="sm" onclick={() => openJellyfinEditor()}>
					<Plus class="mr-2 h-4 w-4" />
					Add
				</Button>
			</div>
		</CardHeader>
		<CardContent>
			{#if jellyfins.length === 0}
				<div class="py-8 text-center text-muted-foreground">
					<Film class="mx-auto mb-2 h-12 w-12 opacity-50" />
					<p>No Jellyfin integrations configured</p>
					<p class="mt-1 text-sm">Click "Add" to create one</p>
				</div>
			{:else}
				<div class="space-y-3">
					{#each jellyfins as jellyfin}
						<div class="flex items-center justify-between rounded-lg border p-3">
							<div class="flex items-center gap-3">
								<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
									<Film class="h-5 w-5 text-primary" />
								</div>
								<div>
									<p class="font-medium">{jellyfin.name}</p>
									<p class="text-xs text-muted-foreground">{jellyfin.url}</p>
								</div>
							</div>
							<div class="flex items-center gap-2">
								{#if jellyfin.enabled}
									<Badge variant="default" class="bg-green-500">Enabled</Badge>
								{:else}
									<Badge variant="secondary">Disabled</Badge>
								{/if}
								<Button
									variant="ghost"
									size="icon"
									onclick={() => openJellyfinEditor(jellyfin)}
								>
									<Pencil class="h-4 w-4" />
								</Button>
								<Button
									variant="ghost"
									size="icon"
									onclick={() => handleDeleteJellyfin(jellyfin.name)}
									disabled={deletingJellyfin === jellyfin.name}
								>
									<Trash2
										class="h-4 w-4 {deletingJellyfin === jellyfin.name
											? 'animate-pulse'
											: ''}"
									/>
								</Button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</CardContent>
	</Card>

	<!-- Conversion Settings -->
	<Card>
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<RefreshCw class="h-5 w-5" />
				Conversion Settings
			</CardTitle>
			<CardDescription>Configure automatic media conversion</CardDescription>
		</CardHeader>
		<CardContent>
			<div class="space-y-4">
				<label class="flex cursor-pointer items-center gap-3">
					<input
						type="checkbox"
						bind:checked={conversionSettings.auto_convert_dv_p7_to_p8}
						onchange={handleConversionSettingChange}
						class="h-5 w-5 rounded border-gray-300"
					/>
					<div>
						<p class="font-medium">Auto-convert DV Profile 7 to Profile 8</p>
						<p class="text-sm text-muted-foreground">
							Automatically convert Dolby Vision Profile 7 files to Profile 8 when imported.
							Profile 8 provides better device compatibility (Infuse, Apple TV).
						</p>
					</div>
				</label>
			</div>
		</CardContent>
	</Card>
</div>

<!-- Arr Editor Dialog -->
<Dialog.Root bind:open={arrEditorOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>{editingArr ? 'Edit Arr' : 'Add Arr'}</Dialog.Title>
			<Dialog.Description>Configure Radarr or Sonarr integration</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-4">
			<div class="space-y-2">
				<label for="arr-name" class="text-sm font-medium">Name</label>
				<Input id="arr-name" bind:value={arrForm.name} placeholder="radarr" />
			</div>

			<div class="space-y-2">
				<label class="text-sm font-medium">Type</label>
				<div class="flex gap-4">
					<label class="flex items-center gap-2">
						<input type="radio" bind:group={arrForm.type} value="radarr" />
						<span>Radarr</span>
					</label>
					<label class="flex items-center gap-2">
						<input type="radio" bind:group={arrForm.type} value="sonarr" />
						<span>Sonarr</span>
					</label>
				</div>
			</div>

			<div class="space-y-2">
				<label for="arr-url" class="text-sm font-medium">URL</label>
				<Input id="arr-url" bind:value={arrForm.url} placeholder="http://localhost:7878" />
			</div>

			<div class="space-y-2">
				<label for="arr-api-key" class="text-sm font-medium">
					API Key {editingArr ? '(leave empty to keep current)' : ''}
				</label>
				<Input
					id="arr-api-key"
					type="password"
					bind:value={arrForm.api_key}
					placeholder="Your API key"
				/>
			</div>

			<div class="space-y-2">
				<label class="flex items-center gap-2">
					<input type="checkbox" bind:checked={arrForm.enabled} class="h-4 w-4" />
					<span class="text-sm font-medium">Enabled</span>
				</label>
				<label class="flex items-center gap-2">
					<input type="checkbox" bind:checked={arrForm.auto_rescan} class="h-4 w-4" />
					<span class="text-sm font-medium">Auto Rescan after processing</span>
				</label>
				<label class="flex items-center gap-2">
					<input type="checkbox" bind:checked={arrForm.auto_rename} class="h-4 w-4" />
					<span class="text-sm font-medium">Auto Rename after processing</span>
				</label>
			</div>

			{#if arrError}
				<div class="flex items-center gap-2 text-sm text-destructive">
					<AlertCircle class="h-4 w-4" />
					<span>{arrError}</span>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (arrEditorOpen = false)} disabled={arrLoading}>
				Cancel
			</Button>
			<Button onclick={handleSaveArr} disabled={arrLoading}>
				{#if arrLoading}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					Saving...
				{:else}
					Save
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Jellyfin Editor Dialog -->
<Dialog.Root bind:open={jellyfinEditorOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>{editingJellyfin ? 'Edit Jellyfin' : 'Add Jellyfin'}</Dialog.Title>
			<Dialog.Description>Configure Jellyfin media server</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-4">
			<div class="space-y-2">
				<label for="jellyfin-name" class="text-sm font-medium">Name</label>
				<Input id="jellyfin-name" bind:value={jellyfinForm.name} placeholder="jellyfin" />
			</div>

			<div class="space-y-2">
				<label for="jellyfin-url" class="text-sm font-medium">URL</label>
				<Input
					id="jellyfin-url"
					bind:value={jellyfinForm.url}
					placeholder="http://localhost:8096"
				/>
			</div>

			<div class="space-y-2">
				<label for="jellyfin-api-key" class="text-sm font-medium">
					API Key {editingJellyfin ? '(leave empty to keep current)' : ''}
				</label>
				<Input
					id="jellyfin-api-key"
					type="password"
					bind:value={jellyfinForm.api_key}
					placeholder="Your API key"
				/>
			</div>

			<label class="flex items-center gap-2">
				<input type="checkbox" bind:checked={jellyfinForm.enabled} class="h-4 w-4" />
				<span class="text-sm font-medium">Enabled</span>
			</label>

			{#if jellyfinError}
				<div class="flex items-center gap-2 text-sm text-destructive">
					<AlertCircle class="h-4 w-4" />
					<span>{jellyfinError}</span>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => (jellyfinEditorOpen = false)}
				disabled={jellyfinLoading}
			>
				Cancel
			</Button>
			<Button onclick={handleSaveJellyfin} disabled={jellyfinLoading}>
				{#if jellyfinLoading}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					Saving...
				{:else}
					Save
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Library Editor Dialog -->
<Dialog.Root bind:open={libraryEditorOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Add Library</Dialog.Title>
			<Dialog.Description>Configure a media library to scan</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-4">
			<div class="space-y-2">
				<label for="library-name" class="text-sm font-medium">Name</label>
				<Input id="library-name" bind:value={libraryForm.name} placeholder="My Movies" />
			</div>

			<div class="space-y-2">
				<label class="text-sm font-medium">Media Type</label>
				<div class="flex gap-4">
					<label class="flex items-center gap-2">
						<input type="radio" bind:group={libraryForm.media_type} value="movies" />
						<span>Movies</span>
					</label>
					<label class="flex items-center gap-2">
						<input type="radio" bind:group={libraryForm.media_type} value="tvshows" />
						<span>TV Shows</span>
					</label>
					<label class="flex items-center gap-2">
						<input type="radio" bind:group={libraryForm.media_type} value="music" />
						<span>Music</span>
					</label>
				</div>
			</div>

			<div class="space-y-2">
				<label class="text-sm font-medium">Paths</label>
				{#each libraryForm.paths as _, i}
					<div class="flex gap-2">
						<Input
							bind:value={libraryForm.paths[i]}
							placeholder="/media/movies"
							class="flex-1"
						/>
						{#if libraryForm.paths.length > 1}
							<Button
								variant="ghost"
								size="icon"
								onclick={() => {
									libraryForm.paths = libraryForm.paths.filter((_, idx) => idx !== i);
								}}
							>
								<Trash2 class="h-4 w-4" />
							</Button>
						{/if}
					</div>
				{/each}
				<Button
					variant="outline"
					size="sm"
					onclick={() => {
						libraryForm.paths = [...libraryForm.paths, ''];
					}}
				>
					<Plus class="mr-2 h-4 w-4" />
					Add Path
				</Button>
				<p class="text-xs text-muted-foreground">
					Add directories containing your media files
				</p>
			</div>

			{#if libraryError}
				<div class="flex items-center gap-2 text-sm text-destructive">
					<AlertCircle class="h-4 w-4" />
					<span>{libraryError}</span>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => (libraryEditorOpen = false)}
				disabled={libraryLoading}
			>
				Cancel
			</Button>
			<Button onclick={handleSaveLibrary} disabled={libraryLoading}>
				{#if libraryLoading}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					Creating...
				{:else}
					Create
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
