<template>
	<div v-if="loading" class="space-y-3">
		<ShadSkeleton v-for="n in 2" :key="n" class="h-20 w-full" />
	</div>
	<div v-else-if="!teams.length" class="rounded-lg border border-dashed border-border bg-card py-10 text-center text-sm text-muted-foreground">
		{{ store.getTranslation('developer.models.no_teams') }}
	</div>
	<div v-else class="space-y-3">
		<div v-for="team in teams" :key="team.id" class="rounded-lg border border-border bg-card p-4">
			<div class="mb-2 flex flex-wrap items-center gap-2">
				<span class="font-semibold text-foreground">{{ team.name }}</span>
				<ShadBadge v-if="team.is_default" variant="secondary">{{ store.getTranslation('developer.models.default_team') }}</ShadBadge>
				<span class="text-xs text-muted-foreground">{{ store.getTranslation('developer.models_count', {count: team.models.length}) }}</span>
			</div>
			<div v-if="team.models.length" class="flex flex-wrap gap-1.5">
				<code v-for="model in team.models" :key="model" class="break-all rounded bg-muted px-2 py-0.5 font-mono text-xs text-foreground">{{ model }}</code>
			</div>
			<p v-else class="text-sm text-muted-foreground">{{ store.getTranslation('developer.no_models') }}</p>
		</div>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';
import type {DeveloperTeam} from '~/types/developer';

defineProps<{teams: DeveloperTeam[]; loading: boolean}>();
const store = useMainStore();
</script>
