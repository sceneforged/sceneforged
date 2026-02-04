<script lang="ts">
	let { children, child, ...rest }: Record<string, any> = $props();
</script>

{#if child}
	{@render child({ props: rest })}
{:else if children}
	{@render children()}
{/if}
