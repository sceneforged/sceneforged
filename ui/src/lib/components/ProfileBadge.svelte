<script lang="ts">
	interface Props {
		profile: string;
		label?: string;
		class?: string;
	}

	let { profile, label, class: className = '' }: Props = $props();

	const colorClass = $derived.by(() => {
		switch (profile.toUpperCase()) {
			case 'A':
				return 'bg-orange-500/15 text-orange-600 border-orange-500/30 dark:text-orange-400';
			case 'B':
				return 'bg-green-500/15 text-green-600 border-green-500/30 dark:text-green-400';
			case 'C':
				return 'bg-blue-500/15 text-blue-600 border-blue-500/30 dark:text-blue-400';
			default:
				return 'bg-muted text-muted-foreground border-border';
		}
	});

	const displayLabel = $derived(label ?? profileLabel(profile));

	function profileLabel(p: string): string {
		switch (p.toUpperCase()) {
			case 'A':
				return 'Source';
			case 'B':
				return 'Universal';
			case 'C':
				return 'Other';
			default:
				return p;
		}
	}
</script>

<span
	class="inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-xs font-semibold {colorClass} {className}"
>
	<span class="font-bold">{profile.toUpperCase()}</span>
	{#if displayLabel}
		<span class="font-medium">{displayLabel}</span>
	{/if}
</span>
