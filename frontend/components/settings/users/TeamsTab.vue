<template>
	<SettingsUsersSection :title="t('teams_title')" :to="canViewTeams ? '/settings/teams' : undefined" :link-label="t('manage_teams')" test-id="user-teams-section">
		<SettingsAssignmentPicker
			:model-value="targets"
			:options="options"
			:kinds="['team']"
			:disabled="!editable || !canViewTeams"
			:description="t('teams_description')"
			:describe="describe"
			:inherited-hint="t('teams_default_hint')"
			:note="t(editable ? 'teams_note' : 'readonly_note')"
			test-id="user-teams"
			@update:model-value="value => emit('replace', value)"
		/>
	</SettingsUsersSection>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';
import type {AssignmentOption, AssignmentTarget} from '~/types/assignments';
import {ADMIN_TEAMS_VIEW} from '~/types/generated/permissions';
import type {UserOverviewResponse} from '~/types/users';

const props = defineProps<{overview: UserOverviewResponse; targets: AssignmentTarget[]; editable: boolean; active: boolean}>();
const emit = defineEmits<{replace: [targets: AssignmentTarget[]]}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`settings.user_detail.${key}`);

const canViewTeams = computed(() => store.hasPermission(ADMIN_TEAMS_VIEW));
const options = ref<AssignmentOption[]>([]);
let loaded = false;

function describe(target: AssignmentOption): string {
	const team = props.overview.teams.find(member => member.id === target.id);
	return team ? t(`team_role.${team.role}`) : '';
}

watch(
	() => props.active,
	async active => {
		if (!active || loaded || !props.editable || !canViewTeams.value) return;
		loaded = true;
		const list = await adminTeams({size: 100});
		options.value = (list?.items ?? []).map(team => ({kind: 'team' as const, id: team.id, label: team.name}));
	},
	{immediate: true},
);
</script>
