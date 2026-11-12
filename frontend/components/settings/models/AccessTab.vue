<template>
	<div class="space-y-3">
		<h3 class="text-base font-semibold text-foreground">{{ t('access') }}</h3>
		<div v-if="!data && active" class="space-y-2">
			<ShadSkeleton class="h-4 w-2/3" />
			<ShadSkeleton class="h-12 w-full" />
			<ShadSkeleton class="h-12 w-full" />
		</div>
		<SettingsAssignmentPicker
			v-else-if="data"
			:model-value="data.targets"
			:options="teams"
			:kinds="data.kinds"
			:disabled="!store.hasPermission(ADMIN_TEAMS_EDIT)"
			:description="t('access_desc')"
			:describe="() => t('access_effect')"
			:inherited-hint="t('access_inherited')"
			inherited-to="/settings/teams"
			:empty-title="t('access_empty_title')"
			:empty-hint="t('access_empty_hint')"
			test-id="model-access"
			@update:model-value="targets => replace(path, targets)"
		/>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';
import type {AssignmentOption} from '~/types/assignments';
import {ADMIN_TEAMS_EDIT} from '~/types/generated/permissions';

const props = defineProps<{modelId: string; active: boolean}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`settings.models.editor.${key}`);
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
