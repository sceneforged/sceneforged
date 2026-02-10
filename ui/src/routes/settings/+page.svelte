<script lang="ts">
	import { themeStore } from '$lib/stores/theme.svelte.js';
	import { preferencesStore } from '$lib/stores/preferences.svelte.js';
	import { authStore } from '$lib/stores/auth.svelte.js';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle,
		CardDescription
	} from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Sun, Moon, Monitor, Play, Subtitles, UserCircle } from '@lucide/svelte';

	type Theme = 'light' | 'dark' | 'system';
	const themes: { value: Theme; label: string; icon: typeof Sun }[] = [
		{ value: 'light', label: 'Light', icon: Sun },
		{ value: 'dark', label: 'Dark', icon: Moon },
		{ value: 'system', label: 'System', icon: Monitor }
	];
</script>

<svelte:head>
	<title>Settings - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<h1 class="text-2xl font-bold">Settings</h1>

	<!-- Appearance -->
	<Card>
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<Sun class="h-5 w-5" />
				Appearance
			</CardTitle>
			<CardDescription>Customize how SceneForged looks</CardDescription>
		</CardHeader>
		<CardContent>
			<div class="space-y-2">
				<label class="text-sm font-medium">Theme</label>
				<div class="flex gap-2">
					{#each themes as t}
						{@const Icon = t.icon}
						<Button
							variant={themeStore.theme === t.value ? 'default' : 'outline'}
							size="sm"
							onclick={() => themeStore.set(t.value)}
						>
							<Icon class="mr-2 h-4 w-4" />
							{t.label}
						</Button>
					{/each}
				</div>
			</div>
		</CardContent>
	</Card>

	<!-- Playback -->
	<Card>
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<Play class="h-5 w-5" />
				Playback
			</CardTitle>
			<CardDescription>Control playback behavior</CardDescription>
		</CardHeader>
		<CardContent>
			<div class="space-y-4">
				<label class="flex cursor-pointer items-center gap-3">
					<input
						type="checkbox"
						bind:checked={preferencesStore.autoplayNextEpisode}
						class="h-5 w-5 rounded border-gray-300"
					/>
					<div>
						<p class="font-medium">Autoplay next episode</p>
						<p class="text-sm text-muted-foreground">
							Automatically play the next episode when the current one finishes.
						</p>
					</div>
				</label>

				<div class="space-y-2">
					<label for="subtitle-lang" class="text-sm font-medium flex items-center gap-2">
						<Subtitles class="h-4 w-4" />
						Default subtitle language
					</label>
					<select
						id="subtitle-lang"
						class="flex h-9 w-full max-w-xs rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm"
						bind:value={preferencesStore.defaultSubtitleLanguage}
					>
						<option value="">None</option>
						<option value="eng">English</option>
						<option value="spa">Spanish</option>
						<option value="fra">French</option>
						<option value="deu">German</option>
						<option value="ita">Italian</option>
						<option value="por">Portuguese</option>
						<option value="jpn">Japanese</option>
						<option value="kor">Korean</option>
						<option value="zho">Chinese</option>
					</select>
				</div>
			</div>
		</CardContent>
	</Card>

	<!-- Account Quick Link -->
	<Card>
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<UserCircle class="h-5 w-5" />
				Account
			</CardTitle>
			<CardDescription>Manage your account</CardDescription>
		</CardHeader>
		<CardContent>
			<div class="flex items-center justify-between">
				<div>
					{#if authStore.username}
						<p class="font-medium">{authStore.username}</p>
					{/if}
					{#if authStore.role}
						<Badge variant="secondary" class="mt-1">{authStore.role}</Badge>
					{/if}
				</div>
				<a href="/account">
					<Button variant="outline" size="sm">Manage Account</Button>
				</a>
			</div>
		</CardContent>
	</Card>
</div>
