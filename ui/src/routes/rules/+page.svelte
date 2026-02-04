<script lang="ts">
	import { onMount } from 'svelte';
	import { getConfigRules, updateConfigRules } from '$lib/api/index.js';
	import type { Rule } from '$lib/types.js';
	import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import {
		BookOpen,
		RefreshCw,
		CheckCircle,
		XCircle,
		ChevronRight,
		Plus,
		Pencil,
		Trash2,
		Save,
		X
	} from '@lucide/svelte';

	let rules = $state<Rule[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Editor state
	let editorOpen = $state(false);
	let editingIndex = $state<number | null>(null);
	let editorName = $state('');
	let editorEnabled = $state(true);
	let editorPriority = $state(0);

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

	async function handleToggleEnabled(index: number) {
		const updated = rules.map((r, i) => (i === index ? { ...r, enabled: !r.enabled } : r));
		try {
			rules = await updateConfigRules(updated);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to update rule';
		}
	}

	async function handleDeleteRule(index: number) {
		const ruleName = rules[index]?.name;
		if (!confirm(`Delete rule "${ruleName}"?`)) return;

		const updated = rules.filter((_, i) => i !== index);
		try {
			rules = await updateConfigRules(updated);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete rule';
		}
	}

	function openEditor(index: number | null = null) {
		editingIndex = index;
		if (index !== null && rules[index]) {
			const rule = rules[index];
			editorName = rule.name;
			editorEnabled = rule.enabled;
			editorPriority = rule.priority;
		} else {
			editorName = '';
			editorEnabled = true;
			editorPriority = rules.length > 0 ? Math.max(...rules.map((r) => r.priority)) + 1 : 1;
		}
		editorOpen = true;
	}

	async function handleSaveRule() {
		if (!editorName.trim()) return;

		const updatedRule: Rule = {
			id: editingIndex !== null && rules[editingIndex] ? rules[editingIndex].id : '',
			name: editorName.trim(),
			enabled: editorEnabled,
			priority: editorPriority,
			expr: editingIndex !== null && rules[editingIndex] ? rules[editingIndex].expr : {},
			actions: editingIndex !== null && rules[editingIndex] ? rules[editingIndex].actions : []
		};

		let updated: Rule[];
		if (editingIndex !== null) {
			updated = rules.map((r, i) => (i === editingIndex ? updatedRule : r));
		} else {
			updated = [...rules, updatedRule];
		}

		try {
			rules = await updateConfigRules(updated);
			editorOpen = false;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save rule';
		}
	}

	function formatConditions(rule: Rule): string[] {
		if (!rule.expr) return [];
		// expr is a recursive expression tree; show a compact JSON summary
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
	});
</script>

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

	<!-- Editor dialog -->
	{#if editorOpen}
		<Card class="border-primary">
			<CardHeader>
				<CardTitle>{editingIndex !== null ? 'Edit Rule' : 'New Rule'}</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="space-y-4">
					<div class="space-y-2">
						<label for="rule-name" class="text-sm font-medium">Name</label>
						<Input id="rule-name" bind:value={editorName} placeholder="Rule name" />
					</div>
					<div class="space-y-2">
						<label for="rule-priority" class="text-sm font-medium">Priority</label>
						<Input
							id="rule-priority"
							type="number"
							bind:value={editorPriority}
						/>
					</div>
					<label class="flex items-center gap-2">
						<input type="checkbox" bind:checked={editorEnabled} class="h-4 w-4" />
						<span class="text-sm font-medium">Enabled</span>
					</label>
					<div class="flex gap-2">
						<Button onclick={handleSaveRule}>
							<Save class="mr-2 h-4 w-4" />
							Save
						</Button>
						<Button variant="outline" onclick={() => (editorOpen = false)}>
							<X class="mr-2 h-4 w-4" />
							Cancel
						</Button>
					</div>
				</div>
			</CardContent>
		</Card>
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
										<button onclick={() => handleToggleEnabled(index)}>
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
									onclick={() => openEditor(index)}
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
					</CardHeader>
					<CardContent>
						<div class="grid gap-6 md:grid-cols-2">
							<div class="space-y-3">
								<h4 class="text-sm font-medium uppercase tracking-wide text-muted-foreground">
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
								<h4 class="text-sm font-medium uppercase tracking-wide text-muted-foreground">
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
