<template>
	<div class="w-full space-y-4">
		<div>
			<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.models.title') }}</h2>
			<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.models.description') }}</p>
		</div>
		<div v-if="developer.error" class="flex items-center justify-between gap-3 rounded-lg border border-border bg-card p-4">
			<p class="text-sm text-foreground">{{ store.getTranslation('developer.errors.load') }}</p>
			<ShadButton variant="outline" size="sm" @click="developer.load().catch(() => {})">{{ store.getTranslation('common.refresh') }}</ShadButton>
		</div>
		<DeveloperTeamModels :teams="developer.teams" :loading="developer.loading && !developer.loaded" />
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';

const store = useMainStore();
const developer = useDeveloperStore();

onMounted(() => {
	developer.load().catch(() => {});
});
</script>
