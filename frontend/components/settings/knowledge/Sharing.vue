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
			:note="t(perRecord ? 'note_per_record' : 'note')"
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

const props = defineProps<{id: string; perRecord?: boolean; managed?: boolean}>();
const store = useMainStore();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`knowledge.sharing.${key}`, params);
const {data, load, replace} = useAssignments();
const path = knowledgeAssignmentsPath(props.id);
const {teams, roles, users, searchUsers, load: loadOptions} = useAssignmentOptions();
const pending = ref<AssignmentTarget[] | null>(null);

const options = computed<AssignmentOption[]>(() => [{kind: 'everyone', id: null, label: ''}, ...teams.value, ...roles.value, ...users.value]);

const key = (target: AssignmentOption) => `${target.kind}:${target.id ?? ''}`;

function describe(target: AssignmentOption) {
	const editor = 'role' in target && target.role === 'editor';
	return t(`effect.${target.kind}${editor ? '_editor' : ''}`, {name: target.label});
}

function change(targets: AssignmentTarget[]) {
	const known = new Set((data.value?.targets ?? []).map(key));
	const broad = !props.managed && !props.perRecord && targets.some(target => target.kind !== 'user' && !known.has(key(target)));
	if (broad) pending.value = targets;
	else replace(path, targets);
}

function confirm() {
	const targets = pending.value;
	pending.value = null;
	if (targets) replace(path, targets);
}

onMounted(async () => {
	await load(path);
	await loadOptions();
});
</script>
