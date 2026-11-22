<template>
	<div class="space-y-3">
		<h3 class="text-base font-semibold text-foreground">{{ t('title') }}</h3>
		<SettingsAssignmentPicker
			:model-value="data?.targets ?? []"
			:options="options"
			:kinds="data?.kinds ?? []"
			:roles="data?.roles ?? []"
			:disabled="!data?.kinds.length"
			:description="t('description')"
			:describe="describe"
			:empty-hint="t('empty')"
			:note="t('note')"
			test-id="knowledge-sharing"
			@update:model-value="change"
			@search="searchUsers"
		/>

		<ShadDialog :open="pending !== null" @update:open="open => !open && (pending = null)">
			<ShadDialogContent>
				<ShadDialogHeader>
					<ShadDialogTitle>{{ t('confirm_title') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ t('confirm_description') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="pending = null">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton data-testid="knowledge-sharing-confirm" @click="confirm">{{ t('confirm') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';
import type {AssignmentOption, AssignmentTarget} from '~/types/assignments';
import {ADMIN_TEAMS_VIEW, ADMIN_USERS_VIEW} from '~/types/generated/permissions';

const props = defineProps<{id: string}>();
const store = useMainStore();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`knowledge.sharing.${key}`, params);
const {data, load, replace} = useAssignments();
const path = knowledgeAssignmentsPath(props.id);
const teams = ref<AssignmentOption[]>([]);
const users = ref<AssignmentOption[]>([]);
const pending = ref<AssignmentTarget[] | null>(null);

const options = computed<AssignmentOption[]>(() => [
	{kind: 'everyone', id: null, label: ''},
	...teams.value,
	...store.roles.map(role => ({kind: 'role' as const, id: role.id, label: role.name})),
	...users.value,
]);

const key = (target: AssignmentOption) => `${target.kind}:${target.id ?? ''}`;

function describe(target: AssignmentOption) {
	const editor = 'role' in target && target.role === 'editor';
	return t(`effect.${target.kind}${editor ? '_editor' : ''}`, {name: target.label});
}

function change(targets: AssignmentTarget[]) {
	const known = new Set((data.value?.targets ?? []).map(key));
	const broad = targets.some(target => target.kind !== 'user' && !known.has(key(target)));
	if (broad) pending.value = targets;
	else replace(path, targets);
}

function confirm() {
	const targets = pending.value;
	pending.value = null;
	if (targets) replace(path, targets);
}

let term = '';
async function searchUsers(search = '') {
	if (!store.hasPermission(ADMIN_USERS_VIEW)) return;
	term = search;
	const list = await adminUsers({page: 1, size: 50, ...(search ? {search} : {})});
	if (term === search) users.value = list.items.map(user => ({kind: 'user' as const, id: user.id, label: user.username || user.email}));
}

onMounted(async () => {
	await load(path);
	if (store.hasPermission(ADMIN_TEAMS_VIEW)) {
		const all: AssignmentOption[] = [];
		for (let page = 1; ; page++) {
			const list = await adminTeams({page, size: 100});
			all.push(...(list?.items ?? []).map(team => ({kind: 'team' as const, id: team.id, label: team.name})));
			if (!list?.has_more) break;
		}
		teams.value = all;
	}
	await searchUsers();
});
</script>
