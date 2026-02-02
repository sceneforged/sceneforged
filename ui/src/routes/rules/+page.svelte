<script lang="ts">
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import {
    BookOpen,
    RefreshCw,
    CheckCircle,
    XCircle,
    ChevronRight,
    Plus,
    Pencil,
    Trash2
  } from 'lucide-svelte';
  import { getConfigRules, createRule, updateRule, deleteRule } from '$lib/api';
  import RuleEditor from '$lib/components/RuleEditor.svelte';
  import type { Rule, Action } from '$lib/types';

  let rules = $state<Rule[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let deletingRule = $state<string | null>(null);

  // Editor state
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

  async function handleSaveRule(rule: Omit<Rule, 'normalized'>) {
    try {
      if (editingRule) {
        await updateRule(editingRule.name, rule);
        toast.success('Rule saved');
      } else {
        await createRule(rule);
        toast.success('Rule created');
      }
      await loadData();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to save rule');
    }
  }

  async function handleDeleteRule(name: string) {
    if (!confirm(`Delete rule "${name}"?`)) return;

    deletingRule = name;
    try {
      await deleteRule(name);
      await loadData();
      toast.success('Rule deleted');
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete rule';
    } finally {
      deletingRule = null;
    }
  }

  async function handleToggleEnabled(rule: Rule) {
    try {
      const newEnabled = !rule.enabled;
      await updateRule(rule.name, { ...rule, enabled: newEnabled });
      await loadData();
      toast.success(newEnabled ? 'Rule enabled' : 'Rule disabled');
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to update rule';
    }
  }

  function openEditor(rule: Rule | null = null) {
    editingRule = rule;
    editorOpen = true;
  }

  function formatAction(action: Action): string {
    switch (action.type) {
      case 'dv_convert':
        return `Convert DV to Profile ${action.target_profile}`;
      case 'remux':
        return `Remux to ${action.container.toUpperCase()}`;
      case 'add_compat_audio':
        return `Add ${action.target_codec} from ${action.source_codec}`;
      case 'strip_tracks':
        const parts = [];
        if (action.track_types.length) parts.push(`types: ${action.track_types.join(', ')}`);
        if (action.languages.length) parts.push(`langs: ${action.languages.join(', ')}`);
        return `Strip tracks (${parts.join('; ') || 'all'})`;
      case 'exec':
        return `Exec: ${action.command}`;
      default:
        return 'Unknown action';
    }
  }

  function formatConditions(rule: Rule): string[] {
    const conditions: string[] = [];
    const match = rule.match_conditions;

    if (match.codecs.length) {
      conditions.push(`Codec: ${match.codecs.join(', ')}`);
    }
    if (match.containers.length) {
      conditions.push(`Container: ${match.containers.join(', ')}`);
    }
    if (match.hdr_formats.length) {
      conditions.push(`HDR: ${match.hdr_formats.join(', ')}`);
    }
    if (match.dolby_vision_profiles.length) {
      conditions.push(`DV Profile: ${match.dolby_vision_profiles.join(', ')}`);
    }
    if (match.audio_codecs.length) {
      conditions.push(`Audio: ${match.audio_codecs.join(', ')}`);
    }
    if (match.min_resolution) {
      conditions.push(`Min Res: ${match.min_resolution.width}x${match.min_resolution.height}`);
    }
    if (match.max_resolution) {
      conditions.push(`Max Res: ${match.max_resolution.width}x${match.max_resolution.height}`);
    }

    return conditions;
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
        <Plus class="h-4 w-4 mr-2" />
        New Rule
      </Button>
      <Button variant="outline" size="sm" onclick={loadData} disabled={loading}>
        <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
        Refresh
      </Button>
    </div>
  </div>

  {#if error}
    <div class="bg-destructive/10 text-destructive p-4 rounded-md">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="text-center py-12 text-muted-foreground">
      <RefreshCw class="h-8 w-8 mx-auto mb-2 animate-spin" />
      <p>Loading rules...</p>
    </div>
  {:else if rules.length === 0}
    <Card>
      <CardContent class="py-12">
        <div class="text-center text-muted-foreground">
          <BookOpen class="h-12 w-12 mx-auto mb-4 opacity-50" />
          <p class="text-lg font-medium">No rules configured</p>
          <p class="text-sm mt-2">Click "New Rule" to create your first processing rule</p>
        </div>
      </CardContent>
    </Card>
  {:else}
    <div class="space-y-4">
      {#each rules.sort((a, b) => b.priority - a.priority) as rule, index}
        <Card class={rule.enabled ? '' : 'opacity-60'}>
          <CardHeader>
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-8 h-8 rounded-full bg-primary/10 text-primary font-bold text-sm">
                  {index + 1}
                </div>
                <div>
                  <CardTitle class="text-lg flex items-center gap-2">
                    {rule.name}
                    <button onclick={() => handleToggleEnabled(rule)}>
                      {#if rule.enabled}
                        <Badge variant="default" class="bg-green-500 cursor-pointer">
                          <CheckCircle class="h-3 w-3 mr-1" />
                          Active
                        </Badge>
                      {:else}
                        <Badge variant="secondary" class="cursor-pointer">
                          <XCircle class="h-3 w-3 mr-1" />
                          Disabled
                        </Badge>
                      {/if}
                    </button>
                  </CardTitle>
                  <CardDescription>Priority: {rule.priority}</CardDescription>
                </div>
              </div>
              <div class="flex items-center gap-2">
                <Button variant="ghost" size="icon" onclick={() => openEditor(rule)} title="Edit">
                  <Pencil class="h-4 w-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onclick={() => handleDeleteRule(rule.name)}
                  disabled={deletingRule === rule.name}
                  title="Delete"
                >
                  <Trash2 class="h-4 w-4 {deletingRule === rule.name ? 'animate-pulse' : ''}" />
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div class="grid md:grid-cols-2 gap-6">
              <!-- Match Conditions -->
              <div class="space-y-3">
                <h4 class="font-medium text-sm text-muted-foreground uppercase tracking-wide">
                  Match Conditions
                </h4>
                {#if formatConditions(rule).length === 0}
                  <p class="text-sm text-muted-foreground italic">Matches all files</p>
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

              <!-- Actions -->
              <div class="space-y-3">
                <h4 class="font-medium text-sm text-muted-foreground uppercase tracking-wide">
                  Actions ({rule.actions.length})
                </h4>
                <ol class="space-y-2">
                  {#each rule.actions as action, i}
                    <li class="flex items-center gap-2 text-sm">
                      <span class="flex items-center justify-center w-5 h-5 rounded-full bg-secondary text-secondary-foreground text-xs">
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

    <Card class="bg-muted/50">
      <CardContent class="py-4">
        <p class="text-sm text-muted-foreground text-center">
          {rules.length} rule{rules.length === 1 ? '' : 's'} configured •
          {rules.filter(r => r.enabled).length} active •
          Changes are saved automatically
        </p>
      </CardContent>
    </Card>
  {/if}
</div>

<RuleEditor
  bind:open={editorOpen}
  rule={editingRule}
  onSave={handleSaveRule}
  onClose={() => { editingRule = null; }}
/>
