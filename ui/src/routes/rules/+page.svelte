<script lang="ts">
	import { onMount } from 'svelte';
	import AuthGuard from '$lib/components/AuthGuard.svelte';
	import { getConfigRules, createRule, updateRule, deleteRule } from '$lib/api/index.js';
	import type { Rule } from '$lib/types.js';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle,
		CardDescription
	} from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import RuleEditor from '$lib/components/RuleEditor.svelte';
	import {
		BookOpen,
		RefreshCw,
		CheckCircle,
		XCircle,
		ChevronRight,
		Plus,
		Pencil,
		Trash2
	} from '@lucide/svelte';

	let rules = $state<Rule[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Rule editor state
	let editorOpen = $state(false);
	let editingRule = $state<Rule | null>(null);

	async function loadData() {
		loading = true;
		error = null;
		try {
			rules = await getConfigRules();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load rules';
		} finally {
			loading = false;
		}
	}

	async function handleToggleEnabled(rule: Rule) {
		try {
			await updateRule(rule.name, {
				...rule,
				enabled: !rule.enabled
			});
			rules = rules.map((r) =>
				r.id === rule.id ? { ...r, enabled: !r.enabled } : r
			);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to update rule';
		}
	}

	async function handleDeleteRule(rule: Rule) {
		if (!confirm(`Delete rule "${rule.name}"?`)) return;
		try {
			await deleteRule(rule.name);
			rules = rules.filter((r) => r.id !== rule.id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete rule';
		}
	}

	function openEditor(rule: Rule | null = null) {
		editingRule = rule;
		editorOpen = true;
	}

	async function handleSaveRule(ruleData: Omit<Rule, 'id'>) {
		if (editingRule) {
			// Update existing rule
			const updated = await updateRule(editingRule.name, ruleData);
			rules = rules.map((r) => (r.id === editingRule!.id ? updated : r));
		} else {
			// Create new rule
			const created = await createRule(ruleData);
			rules = [...rules, created];
		}
	}

	function formatConditions(rule: Rule): string[] {
		if (!rule.match_conditions) return [];
		const conditions: string[] = [];
		const mc = rule.match_conditions;
		if (mc.codecs?.length > 0) conditions.push(`codecs: ${mc.codecs.join(', ')}`);
		if (mc.containers?.length > 0) conditions.push(`containers: ${mc.containers.join(', ')}`);
		if (mc.hdr_formats?.length > 0) conditions.push(`hdr: ${mc.hdr_formats.join(', ')}`);
		if (mc.dolby_vision_profiles?.length > 0)
			conditions.push(`dv profiles: ${mc.dolby_vision_profiles.join(', ')}`);
		if (mc.audio_codecs?.length > 0) conditions.push(`audio: ${mc.audio_codecs.join(', ')}`);
		if (mc.min_resolution)
			conditions.push(`min: ${mc.min_resolution.width}x${mc.min_resolution.height}`);
		if (mc.max_resolution)
			conditions.push(`max: ${mc.max_resolution.width}x${mc.max_resolution.height}`);
		return conditions;
	}

	function formatAction(action: Record<string, unknown>): string {
		const type = action.type as string;
		if (!type) return 'Unknown action';
		return type
			.replace(/_/g, ' ')
			.replace(/\b\w/g, (l) => l.toUpperCase());
	}

	onMount(() => {
		loadData();
	});
</script>

<AuthGuard requireAdmin>
<div class="space-y-6">
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold">Rules</h1>
		<div class="flex items-center gap-2">
			<Button variant="default" size="sm" onclick={() => openEditor()}>
				<Plus class="mr-2 h-4 w-4" />
				New Rule
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

	{#if loading}
		<div class="py-12 text-center text-muted-foreground">
			<RefreshCw class="mx-auto mb-2 h-8 w-8 animate-spin" />
			<p>Loading rules...</p>
		</div>
	{:else if rules.length === 0}
		<Card>
			<CardContent class="py-12">
				<div class="text-center text-muted-foreground">
					<BookOpen class="mx-auto mb-4 h-12 w-12 opacity-50" />
					<p class="text-lg font-medium">No rules configured</p>
					<p class="mt-2 text-sm">Click "New Rule" to create your first processing rule</p>
				</div>
			</CardContent>
		</Card>
	{:else}
		<div class="space-y-4">
			{#each [...rules].sort((a, b) => b.priority - a.priority) as rule, index}
				<Card class={rule.enabled ? '' : 'opacity-60'}>
					<CardHeader>
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-3">
								<div
									class="flex h-8 w-8 items-center justify-center rounded-full bg-primary/10 text-sm font-bold text-primary"
								>
									{index + 1}
								</div>
								<div>
									<CardTitle class="flex items-center gap-2 text-lg">
										{rule.name}
										<button onclick={() => handleToggleEnabled(rule)}>
											{#if rule.enabled}
												<Badge variant="default" class="cursor-pointer bg-green-500">
													<CheckCircle class="mr-1 h-3 w-3" />
													Active
												</Badge>
											{:else}
												<Badge variant="secondary" class="cursor-pointer">
													<XCircle class="mr-1 h-3 w-3" />
													Disabled
												</Badge>
											{/if}
										</button>
									</CardTitle>
									<CardDescription>Priority: {rule.priority}</CardDescription>
								</div>
							</div>
							<div class="flex items-center gap-2">
								<Button
									variant="ghost"
									size="icon"
									onclick={() => openEditor(rule)}
									title="Edit"
								>
									<Pencil class="h-4 w-4" />
								</Button>
								<Button
									variant="ghost"
									size="icon"
									onclick={() => handleDeleteRule(rule)}
									title="Delete"
								>
									<Trash2 class="h-4 w-4" />
								</Button>
							</div>
						</div>
					</CardHeader>
					<CardContent>
						<div class="grid gap-6 md:grid-cols-2">
							<div class="space-y-3">
								<h4
									class="text-sm font-medium uppercase tracking-wide text-muted-foreground"
								>
									Match Conditions
								</h4>
								{#if formatConditions(rule).length === 0}
									<p class="text-sm italic text-muted-foreground">Matches all files</p>
								{:else}
									<ul class="space-y-1">
										{#each formatConditions(rule) as condition}
											<li class="flex items-center gap-2 text-sm">
												<ChevronRight class="h-4 w-4 text-muted-foreground" />
												{condition}
											</li>
										{/each}
									</ul>
								{/if}
							</div>

							<div class="space-y-3">
								<h4
									class="text-sm font-medium uppercase tracking-wide text-muted-foreground"
								>
									Actions ({rule.actions.length})
								</h4>
								<ol class="space-y-2">
									{#each rule.actions as action, i}
										<li class="flex items-center gap-2 text-sm">
											<span
												class="flex h-5 w-5 items-center justify-center rounded-full bg-secondary text-xs text-foreground"
											>
												{i + 1}
											</span>
											<span>{formatAction(action)}</span>
										</li>
									{/each}
								</ol>
							</div>
						</div>
					</CardContent>
				</Card>
			{/each}
		</div>
	{/if}
</div>

<RuleEditor
	bind:open={editorOpen}
	rule={editingRule}
	onsave={handleSaveRule}
	onclose={() => {
		editingRule = null;
	}}
/>
</AuthGuard>
