<template>
	<div class="space-y-5">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">{{ store.getTranslation('settings.models.editor.access') }}</h3>
			<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.models.editor.access_desc') }}</p>
		</div>
		<div v-if="!data" class="flex items-center justify-center py-10 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>
		<SettingsAssignmentPicker
			v-else
			:model-value="data.targets"
			:options="teams"
			:kinds="data.kinds"
			:disabled="!store.hasPermission(ADMIN_TEAMS_EDIT)"
			test-id="model-access"
			@update:model-value="targets => replace(path, targets)"
		/>
	</div>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AssignmentOption} from '~/types/assignments';
import {ADMIN_TEAMS_EDIT} from '~/types/generated/permissions';

const props = defineProps<{modelId: string; active: boolean}>();

const store = useMainStore();
const {data, load, replace} = useAssignments();
const teams = ref<AssignmentOption[]>([]);
const path = `/api/v1/admin/models/${props.modelId}/assignments`;

watch(
	() => props.active,
	async active => {
		if (!active || data.value) return;
		const [, list] = await Promise.all([load(path), adminTeams({size: 200})]);
		teams.value = (list?.items ?? []).map(team => ({kind: 'team' as const, id: team.id, label: team.name}));
	},
	{immediate: true},
);
</script>
