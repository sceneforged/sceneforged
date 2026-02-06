<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Loader2, AlertCircle, Plus, X } from '@lucide/svelte';
	import type { Rule, ActionConfig, MatchConditions } from '$lib/types.js';

	interface Props {
		open: boolean;
		rule: Rule | null;
		onsave: (rule: Omit<Rule, 'id'>) => Promise<void>;
		onclose: () => void;
	}

	let { open = $bindable(), rule, onsave, onclose }: Props = $props();

	let loading = $state(false);
	let error = $state<string | null>(null);

	// Form state
	let name = $state('');
	let enabled = $state(true);
	let priority = $state(50);
	let codecs = $state<string[]>([]);
	let containers = $state<string[]>([]);
	let hdr_formats = $state<string[]>([]);
	let dolby_vision_profiles = $state<number[]>([]);
	let audio_codecs = $state<string[]>([]);
	let min_resolution = $state<{ width: number; height: number } | null>(null);
	let max_resolution = $state<{ width: number; height: number } | null>(null);
	let actions = $state<ActionConfig[]>([]);

	// Input fields for adding items
	let newCodec = $state('');
	let newContainer = $state('');
	let newHdrFormat = $state('');
	let newDvProfile = $state('');
	let newAudioCodec = $state('');

	// Initialize form when rule changes or dialog opens
	$effect(() => {
		if (open) {
			if (rule) {
				name = rule.name;
				enabled = rule.enabled;
				priority = rule.priority;
				codecs = [...(rule.match_conditions?.codecs ?? [])];
				containers = [...(rule.match_conditions?.containers ?? [])];
				hdr_formats = [...(rule.match_conditions?.hdr_formats ?? [])];
				dolby_vision_profiles = [...(rule.match_conditions?.dolby_vision_profiles ?? [])];
				audio_codecs = [...(rule.match_conditions?.audio_codecs ?? [])];
				min_resolution = rule.match_conditions?.min_resolution
					? { ...rule.match_conditions.min_resolution }
					: null;
				max_resolution = rule.match_conditions?.max_resolution
					? { ...rule.match_conditions.max_resolution }
					: null;
				actions = JSON.parse(JSON.stringify(rule.actions));
			} else {
				name = '';
				enabled = true;
				priority = 50;
				codecs = [];
				containers = [];
				hdr_formats = [];
				dolby_vision_profiles = [];
				audio_codecs = [];
				min_resolution = null;
				max_resolution = null;
				actions = [];
			}
			error = null;
		}
	});

	function addItem(list: string[], value: string, reset: () => void) {
		const trimmed = value.trim().toLowerCase();
		if (trimmed && !list.includes(trimmed)) {
			list.push(trimmed);
			reset();
		}
	}

	function addAction(type: string) {
		switch (type) {
			case 'dv_convert':
				actions.push({ type: 'dv_convert', target_profile: 8 });
				break;
			case 'remux':
				actions.push({ type: 'remux', container: 'mkv', keep_original: false });
				break;
			case 'add_compat_audio':
				actions.push({
					type: 'add_compat_audio',
					source_codec: 'truehd',
					target_codec: 'aac'
				});
				break;
			case 'strip_tracks':
				actions.push({ type: 'strip_tracks', track_types: [], languages: [] });
				break;
			case 'exec':
				actions.push({ type: 'exec', command: '', args: [] });
				break;
		}
		actions = actions;
	}

	async function handleSubmit() {
		if (!name.trim()) {
			error = 'Name is required';
			return;
		}

		if (enabled && actions.length === 0) {
			error = 'Enabled rules must have at least one action';
			return;
		}

		loading = true;
		error = null;

		try {
			await onsave({
				name: name.trim(),
				enabled,
				priority,
				match_conditions: {
					codecs,
					containers,
					hdr_formats,
					dolby_vision_profiles,
					audio_codecs,
					min_resolution,
					max_resolution
				},
				actions
			});
			open = false;
			onclose();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save rule';
		} finally {
			loading = false;
		}
	}

	function handleClose() {
		open = false;
		onclose();
	}
</script>

<Dialog.Root bind:open onOpenChange={(isOpen) => !isOpen && handleClose()}>
	<Dialog.Content class="max-h-[90vh] max-w-2xl overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>{rule ? 'Edit Rule' : 'Create Rule'}</Dialog.Title>
			<Dialog.Description>
				{rule ? 'Modify the rule settings' : 'Create a new processing rule'}
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-6 py-4">
			<!-- Basic Info -->
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<label for="rule-name" class="text-sm font-medium">Name</label>
					<Input id="rule-name" bind:value={name} placeholder="my_rule" disabled={loading} />
				</div>
				<div class="space-y-2">
					<label for="rule-priority" class="text-sm font-medium">Priority</label>
					<Input
						id="rule-priority"
						type="number"
						bind:value={priority}
						disabled={loading}
					/>
				</div>
			</div>

			<div class="flex items-center gap-2">
				<input
					type="checkbox"
					id="rule-enabled"
					bind:checked={enabled}
					disabled={loading}
					class="h-4 w-4 rounded border-input"
				/>
				<label for="rule-enabled" class="text-sm font-medium">Enabled</label>
			</div>

			<!-- Match Conditions -->
			<div class="space-y-4">
				<h4 class="text-sm font-medium text-muted-foreground">Match Conditions</h4>

				<!-- Video Codecs -->
				<div class="space-y-2">
					<label class="text-sm">Video Codecs</label>
					<div class="flex gap-2">
						<Input
							placeholder="hevc, h264..."
							bind:value={newCodec}
							onkeydown={(e: KeyboardEvent) =>
								e.key === 'Enter' &&
								addItem(codecs, newCodec, () => (newCodec = ''))}
						/>
						<Button
							size="sm"
							variant="secondary"
							onclick={() => addItem(codecs, newCodec, () => (newCodec = ''))}
						>
							<Plus class="h-4 w-4" />
						</Button>
					</div>
					<div class="flex flex-wrap gap-1">
						{#each codecs as codec, i}
							<Badge variant="secondary" class="gap-1">
								{codec}
								<button
									onclick={() => {
										codecs = codecs.filter((_, idx) => idx !== i);
									}}
								>
									<X class="h-3 w-3" />
								</button>
							</Badge>
						{/each}
					</div>
				</div>

				<!-- Containers -->
				<div class="space-y-2">
					<label class="text-sm">Containers</label>
					<div class="flex gap-2">
						<Input
							placeholder="mkv, avi..."
							bind:value={newContainer}
							onkeydown={(e: KeyboardEvent) =>
								e.key === 'Enter' &&
								addItem(containers, newContainer, () => (newContainer = ''))}
						/>
						<Button
							size="sm"
							variant="secondary"
							onclick={() =>
								addItem(containers, newContainer, () => (newContainer = ''))}
						>
							<Plus class="h-4 w-4" />
						</Button>
					</div>
					<div class="flex flex-wrap gap-1">
						{#each containers as container, i}
							<Badge variant="secondary" class="gap-1">
								{container}
								<button
									onclick={() => {
										containers = containers.filter((_, idx) => idx !== i);
									}}
								>
									<X class="h-3 w-3" />
								</button>
							</Badge>
						{/each}
					</div>
				</div>

				<!-- HDR Formats -->
				<div class="space-y-2">
					<label class="text-sm">HDR Formats</label>
					<div class="flex gap-2">
						<Input
							placeholder="hdr10, hdr10+, dolbyvision..."
							bind:value={newHdrFormat}
							onkeydown={(e: KeyboardEvent) =>
								e.key === 'Enter' &&
								addItem(hdr_formats, newHdrFormat, () => (newHdrFormat = ''))}
						/>
						<Button
							size="sm"
							variant="secondary"
							onclick={() =>
								addItem(hdr_formats, newHdrFormat, () => (newHdrFormat = ''))}
						>
							<Plus class="h-4 w-4" />
						</Button>
					</div>
					<div class="flex flex-wrap gap-1">
						{#each hdr_formats as format, i}
							<Badge variant="secondary" class="gap-1">
								{format}
								<button
									onclick={() => {
										hdr_formats = hdr_formats.filter((_, idx) => idx !== i);
									}}
								>
									<X class="h-3 w-3" />
								</button>
							</Badge>
						{/each}
					</div>
				</div>

				<!-- DV Profiles -->
				<div class="space-y-2">
					<label class="text-sm">Dolby Vision Profiles</label>
					<div class="flex gap-2">
						<Input
							type="number"
							placeholder="5, 7, 8..."
							bind:value={newDvProfile}
							onkeydown={(e: KeyboardEvent) => {
								if (e.key === 'Enter') {
									const num = parseInt(newDvProfile);
									if (!isNaN(num) && !dolby_vision_profiles.includes(num)) {
										dolby_vision_profiles = [...dolby_vision_profiles, num];
										newDvProfile = '';
									}
								}
							}}
						/>
						<Button
							size="sm"
							variant="secondary"
							onclick={() => {
								const num = parseInt(newDvProfile);
								if (!isNaN(num) && !dolby_vision_profiles.includes(num)) {
									dolby_vision_profiles = [...dolby_vision_profiles, num];
									newDvProfile = '';
								}
							}}
						>
							<Plus class="h-4 w-4" />
						</Button>
					</div>
					<div class="flex flex-wrap gap-1">
						{#each dolby_vision_profiles as profile, i}
							<Badge variant="secondary" class="gap-1">
								Profile {profile}
								<button
									onclick={() => {
										dolby_vision_profiles = dolby_vision_profiles.filter(
											(_, idx) => idx !== i
										);
									}}
								>
									<X class="h-3 w-3" />
								</button>
							</Badge>
						{/each}
					</div>
				</div>

				<!-- Audio Codecs -->
				<div class="space-y-2">
					<label class="text-sm">Audio Codecs</label>
					<div class="flex gap-2">
						<Input
							placeholder="truehd, dts-hd..."
							bind:value={newAudioCodec}
							onkeydown={(e: KeyboardEvent) =>
								e.key === 'Enter' &&
								addItem(audio_codecs, newAudioCodec, () => (newAudioCodec = ''))}
						/>
						<Button
							size="sm"
							variant="secondary"
							onclick={() =>
								addItem(audio_codecs, newAudioCodec, () => (newAudioCodec = ''))}
						>
							<Plus class="h-4 w-4" />
						</Button>
					</div>
					<div class="flex flex-wrap gap-1">
						{#each audio_codecs as codec, i}
							<Badge variant="secondary" class="gap-1">
								{codec}
								<button
									onclick={() => {
										audio_codecs = audio_codecs.filter((_, idx) => idx !== i);
									}}
								>
									<X class="h-3 w-3" />
								</button>
							</Badge>
						{/each}
					</div>
				</div>

				<!-- Min/Max Resolution -->
				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<label class="text-sm">Min Resolution</label>
						<div class="flex items-center gap-1">
							<Input
								type="number"
								placeholder="W"
								class="w-20"
								value={min_resolution?.width ?? ''}
								oninput={(e: Event) => {
									const val = parseInt((e.target as HTMLInputElement).value);
									if (!isNaN(val)) {
										min_resolution = {
											width: val,
											height: min_resolution?.height ?? 0
										};
									} else {
										min_resolution = null;
									}
								}}
							/>
							<span class="text-muted-foreground">x</span>
							<Input
								type="number"
								placeholder="H"
								class="w-20"
								value={min_resolution?.height ?? ''}
								oninput={(e: Event) => {
									const val = parseInt((e.target as HTMLInputElement).value);
									if (!isNaN(val)) {
										min_resolution = {
											width: min_resolution?.width ?? 0,
											height: val
										};
									} else {
										min_resolution = null;
									}
								}}
							/>
						</div>
					</div>
					<div class="space-y-2">
						<label class="text-sm">Max Resolution</label>
						<div class="flex items-center gap-1">
							<Input
								type="number"
								placeholder="W"
								class="w-20"
								value={max_resolution?.width ?? ''}
								oninput={(e: Event) => {
									const val = parseInt((e.target as HTMLInputElement).value);
									if (!isNaN(val)) {
										max_resolution = {
											width: val,
											height: max_resolution?.height ?? 0
										};
									} else {
										max_resolution = null;
									}
								}}
							/>
							<span class="text-muted-foreground">x</span>
							<Input
								type="number"
								placeholder="H"
								class="w-20"
								value={max_resolution?.height ?? ''}
								oninput={(e: Event) => {
									const val = parseInt((e.target as HTMLInputElement).value);
									if (!isNaN(val)) {
										max_resolution = {
											width: max_resolution?.width ?? 0,
											height: val
										};
									} else {
										max_resolution = null;
									}
								}}
							/>
						</div>
					</div>
				</div>
			</div>

			<!-- Actions -->
			<div class="space-y-4">
				<div class="flex items-center justify-between">
					<h4 class="text-sm font-medium text-muted-foreground">Actions</h4>
					<div class="flex flex-wrap gap-2">
						<Button size="sm" variant="outline" onclick={() => addAction('dv_convert')}>
							+ DV Convert
						</Button>
						<Button size="sm" variant="outline" onclick={() => addAction('remux')}>
							+ Remux
						</Button>
						<Button
							size="sm"
							variant="outline"
							onclick={() => addAction('add_compat_audio')}
						>
							+ Audio
						</Button>
						<Button size="sm" variant="outline" onclick={() => addAction('strip_tracks')}>
							+ Strip
						</Button>
						<Button size="sm" variant="outline" onclick={() => addAction('exec')}>
							+ Exec
						</Button>
					</div>
				</div>

				{#each actions as action, i}
					<div class="space-y-2 rounded-lg border p-3">
						<div class="flex items-center justify-between">
							<Badge variant="outline">{action.type}</Badge>
							<Button
								size="icon"
								variant="ghost"
								onclick={() => {
									actions = actions.filter((_, idx) => idx !== i);
								}}
							>
								<X class="h-4 w-4" />
							</Button>
						</div>

						{#if action.type === 'dv_convert'}
							<div class="flex items-center gap-2">
								<label class="text-sm">Target Profile:</label>
								<Input type="number" class="w-20" bind:value={action.target_profile} />
							</div>
						{:else if action.type === 'remux'}
							<div class="flex items-center gap-4">
								<div class="flex items-center gap-2">
									<label class="text-sm">Container:</label>
									<Input class="w-24" bind:value={action.container} />
								</div>
								<div class="flex items-center gap-2">
									<input
										type="checkbox"
										checked={!!action.keep_original}
										onchange={(e: Event) => {
											action.keep_original = (e.target as HTMLInputElement).checked;
										}}
										class="h-4 w-4"
									/>
									<label class="text-sm">Keep original</label>
								</div>
							</div>
						{:else if action.type === 'add_compat_audio'}
							<div class="flex items-center gap-4">
								<div class="flex items-center gap-2">
									<label class="text-sm">Source:</label>
									<Input class="w-24" bind:value={action.source_codec} />
								</div>
								<div class="flex items-center gap-2">
									<label class="text-sm">Target:</label>
									<Input class="w-24" bind:value={action.target_codec} />
								</div>
							</div>
						{:else if action.type === 'strip_tracks'}
							<div class="text-xs text-muted-foreground">
								Configure track types and languages to strip
							</div>
						{:else if action.type === 'exec'}
							<div class="space-y-2">
								<div class="flex items-center gap-2">
									<label class="text-sm">Command:</label>
									<Input class="flex-1" bind:value={action.command} placeholder="/usr/bin/cmd" />
								</div>
							</div>
						{/if}
					</div>
				{/each}

				{#if actions.length === 0}
					<p class="py-4 text-center text-sm text-muted-foreground">
						No actions configured. Add an action to process matching files.
					</p>
				{/if}
			</div>

			{#if error}
				<div class="flex items-center gap-2 text-sm text-destructive">
					<AlertCircle class="h-4 w-4" />
					<span>{error}</span>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={handleClose} disabled={loading}>Cancel</Button>
			<Button onclick={handleSubmit} disabled={loading}>
				{#if loading}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					Saving...
				{:else}
					Save Rule
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
