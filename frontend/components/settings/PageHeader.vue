<template>
	<header class="mb-6 flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
		<div class="flex min-w-0 items-center gap-3">
			<slot name="leading" />
			<div class="min-w-0">
				<h2 class="truncate text-lg font-semibold text-foreground">{{ title }}</h2>
				<p v-if="description" class="truncate text-sm text-muted-foreground">{{ description }}</p>
			</div>
		</div>
		<div v-if="$slots.actions" class="flex shrink-0 flex-wrap items-center gap-2">
			<slot name="actions" />
		</div>
	</header>
</template>

<script setup lang="ts">
defineProps<{title: string; description?: string}>();
</script>
