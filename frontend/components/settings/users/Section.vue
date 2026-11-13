<template>
	<section class="space-y-3" :data-testid="testId">
		<div class="flex flex-wrap items-start justify-between gap-x-6 gap-y-1">
			<div class="min-w-0 flex-1 basis-64">
				<h3 class="text-base font-semibold text-foreground">{{ title }}</h3>
				<p v-if="description" class="mt-1 max-w-prose text-xs text-muted-foreground">{{ description }}</p>
			</div>
			<NuxtLink
				v-if="to"
				:to="to"
				class="inline-flex shrink-0 items-center gap-1 rounded-sm text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
				:data-testid="`${testId}-link`"
			>
				{{ linkLabel }}
				<ArrowRight class="h-3.5 w-3.5" />
			</NuxtLink>
		</div>
		<div v-if="emptyTitle" class="rounded-lg border border-dashed border-border px-4 py-6 text-center" :data-testid="`${testId}-empty`">
			<p class="text-sm font-medium text-foreground">{{ emptyTitle }}</p>
			<p v-if="emptyHint" class="mx-auto mt-1 max-w-prose text-xs text-muted-foreground">{{ emptyHint }}</p>
		</div>
		<slot v-else />
	</section>
</template>

<script setup lang="ts">
import {ArrowRight} from 'lucide-vue-next';

withDefaults(defineProps<{title: string; description?: string; to?: string; linkLabel?: string; emptyTitle?: string; emptyHint?: string; testId?: string}>(), {
	description: undefined,
	to: undefined,
	linkLabel: undefined,
	emptyTitle: undefined,
	emptyHint: undefined,
	testId: 'user-section',
});
</script>
