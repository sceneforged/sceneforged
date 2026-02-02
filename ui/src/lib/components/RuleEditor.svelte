<script lang="ts">
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Loader2, AlertCircle, Plus, X, ChevronDown } from 'lucide-svelte';
  import type { Rule, Action, MatchConditions } from '$lib/types';

  interface Props {
    open: boolean;
    rule: Rule | null;
    onSave: (rule: Omit<Rule, 'normalized'>) => Promise<void>;
    onClose: () => void;
  }

  let { open = $bindable(), rule, onSave, onClose }: Props = $props();

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
  let actions = $state<Action[]>([]);

  // Input fields for adding items
  let newCodec = $state('');
  let newContainer = $state('');
  let newHdrFormat = $state('');
  let newDvProfile = $state('');
  let newAudioCodec = $state('');

  // Initialize form when rule changes
  $effect(() => {
    if (open) {
      if (rule) {
        name = rule.name;
        enabled = rule.enabled;
        priority = rule.priority;
        codecs = [...rule.match_conditions.codecs];
        containers = [...rule.match_conditions.containers];
        hdr_formats = [...rule.match_conditions.hdr_formats];
        dolby_vision_profiles = [...rule.match_conditions.dolby_vision_profiles];
        audio_codecs = [...rule.match_conditions.audio_codecs];
        actions = JSON.parse(JSON.stringify(rule.actions));
      } else {
        // Reset for new rule
        name = '';
        enabled = true;
        priority = 50;
        codecs = [];
        containers = [];
        hdr_formats = [];
        dolby_vision_profiles = [];
        audio_codecs = [];
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

  function removeItem<T>(list: T[], index: number) {
    list.splice(index, 1);
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
        actions.push({ type: 'add_compat_audio', source_codec: 'truehd', target_codec: 'aac' });
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
      await onSave({
        name: name.trim(),
        enabled,
        priority,
        match_conditions: {
          codecs,
          containers,
          hdr_formats,
          dolby_vision_profiles,
          audio_codecs,
          min_resolution: null,
          max_resolution: null,
        },
        actions,
      });
      open = false;
      onClose();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save rule';
    } finally {
      loading = false;
    }
  }

  function handleClose() {
    open = false;
    onClose();
  }
</script>

<Dialog bind:open onOpenChange={(isOpen) => !isOpen && handleClose()}>
  <DialogContent class="max-w-2xl max-h-[90vh] overflow-y-auto">
    {#snippet children()}
      <DialogHeader>
        {#snippet children()}
          <DialogTitle>{rule ? 'Edit Rule' : 'Create Rule'}</DialogTitle>
          <DialogDescription>
            {rule ? 'Modify the rule settings' : 'Create a new processing rule'}
          </DialogDescription>
        {/snippet}
      </DialogHeader>

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

          <!-- Codecs -->
          <div class="space-y-2">
            <label class="text-sm">Video Codecs</label>
            <div class="flex gap-2">
              <Input
                placeholder="hevc, h264..."
                bind:value={newCodec}
                onkeydown={(e) => e.key === 'Enter' && addItem(codecs, newCodec, () => newCodec = '')}
              />
              <Button size="sm" variant="secondary" onclick={() => addItem(codecs, newCodec, () => newCodec = '')}>
                <Plus class="h-4 w-4" />
              </Button>
            </div>
            <div class="flex flex-wrap gap-1">
              {#each codecs as codec, i}
                <Badge variant="secondary" class="gap-1">
                  {codec}
                  <button onclick={() => { codecs = codecs.filter((_, idx) => idx !== i); }}>
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
                onkeydown={(e) => e.key === 'Enter' && addItem(containers, newContainer, () => newContainer = '')}
              />
              <Button size="sm" variant="secondary" onclick={() => addItem(containers, newContainer, () => newContainer = '')}>
                <Plus class="h-4 w-4" />
              </Button>
            </div>
            <div class="flex flex-wrap gap-1">
              {#each containers as container, i}
                <Badge variant="secondary" class="gap-1">
                  {container}
                  <button onclick={() => { containers = containers.filter((_, idx) => idx !== i); }}>
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
                onkeydown={(e) => e.key === 'Enter' && addItem(hdr_formats, newHdrFormat, () => newHdrFormat = '')}
              />
              <Button size="sm" variant="secondary" onclick={() => addItem(hdr_formats, newHdrFormat, () => newHdrFormat = '')}>
                <Plus class="h-4 w-4" />
              </Button>
            </div>
            <div class="flex flex-wrap gap-1">
              {#each hdr_formats as format, i}
                <Badge variant="secondary" class="gap-1">
                  {format}
                  <button onclick={() => { hdr_formats = hdr_formats.filter((_, idx) => idx !== i); }}>
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
                onkeydown={(e) => {
                  if (e.key === 'Enter') {
                    const num = parseInt(newDvProfile);
                    if (!isNaN(num) && !dolby_vision_profiles.includes(num)) {
                      dolby_vision_profiles = [...dolby_vision_profiles, num];
                      newDvProfile = '';
                    }
                  }
                }}
              />
              <Button size="sm" variant="secondary" onclick={() => {
                const num = parseInt(newDvProfile);
                if (!isNaN(num) && !dolby_vision_profiles.includes(num)) {
                  dolby_vision_profiles = [...dolby_vision_profiles, num];
                  newDvProfile = '';
                }
              }}>
                <Plus class="h-4 w-4" />
              </Button>
            </div>
            <div class="flex flex-wrap gap-1">
              {#each dolby_vision_profiles as profile, i}
                <Badge variant="secondary" class="gap-1">
                  Profile {profile}
                  <button onclick={() => { dolby_vision_profiles = dolby_vision_profiles.filter((_, idx) => idx !== i); }}>
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
                onkeydown={(e) => e.key === 'Enter' && addItem(audio_codecs, newAudioCodec, () => newAudioCodec = '')}
              />
              <Button size="sm" variant="secondary" onclick={() => addItem(audio_codecs, newAudioCodec, () => newAudioCodec = '')}>
                <Plus class="h-4 w-4" />
              </Button>
            </div>
            <div class="flex flex-wrap gap-1">
              {#each audio_codecs as codec, i}
                <Badge variant="secondary" class="gap-1">
                  {codec}
                  <button onclick={() => { audio_codecs = audio_codecs.filter((_, idx) => idx !== i); }}>
                    <X class="h-3 w-3" />
                  </button>
                </Badge>
              {/each}
            </div>
          </div>
        </div>

        <!-- Actions -->
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <h4 class="text-sm font-medium text-muted-foreground">Actions</h4>
            <div class="flex gap-2">
              <Button size="sm" variant="outline" onclick={() => addAction('dv_convert')}>
                + DV Convert
              </Button>
              <Button size="sm" variant="outline" onclick={() => addAction('remux')}>
                + Remux
              </Button>
              <Button size="sm" variant="outline" onclick={() => addAction('add_compat_audio')}>
                + Audio
              </Button>
            </div>
          </div>

          {#each actions as action, i}
            <div class="p-3 border rounded-lg space-y-2">
              <div class="flex items-center justify-between">
                <Badge variant="outline">{action.type}</Badge>
                <Button size="icon" variant="ghost" onclick={() => { actions = actions.filter((_, idx) => idx !== i); }}>
                  <X class="h-4 w-4" />
                </Button>
              </div>

              {#if action.type === 'dv_convert'}
                <div class="flex items-center gap-2">
                  <label class="text-sm">Target Profile:</label>
                  <Input
                    type="number"
                    class="w-20"
                    bind:value={action.target_profile}
                  />
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
                      bind:checked={action.keep_original}
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
              {/if}
            </div>
          {/each}

          {#if actions.length === 0}
            <p class="text-sm text-muted-foreground text-center py-4">
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

      <DialogFooter>
        {#snippet children()}
          <Button variant="outline" onclick={handleClose} disabled={loading}>
            Cancel
          </Button>
          <Button onclick={handleSubmit} disabled={loading}>
            {#if loading}
              <Loader2 class="h-4 w-4 mr-2 animate-spin" />
              Saving...
            {:else}
              Save Rule
            {/if}
          </Button>
        {/snippet}
      </DialogFooter>
    {/snippet}
  </DialogContent>
</Dialog>
