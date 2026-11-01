<template>
	<div class="border-b border-border py-4 first:pt-0 last:border-b-0 last:pb-0" :class="stacked ? 'space-y-2' : 'flex items-start justify-between gap-4'">
		<div class="min-w-0">
			<p class="text-sm font-medium text-foreground">{{ label }}</p>
			<p v-if="hint" class="mt-0.5 text-xs" :class="warning ? 'text-destructive' : 'text-muted-foreground'">{{ hint }}</p>
		</div>
		<slot />
	</div>
</template>

<script setup lang="ts">
defineProps<{label: string; hint?: string; warning?: boolean; stacked?: boolean}>();
</script>
