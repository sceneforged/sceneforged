<script lang="ts">
	let { children, child, ...rest }: Record<string, any> = $props();
</script>

<!-- Intentionally renders nothing — tooltip content should not appear in DOM during tests -->
