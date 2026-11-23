<template>
	<div class="space-y-4" data-testid="knowledge-access">
		<div>
			<h3 class="text-base font-semibold text-foreground">{{ t('title') }}</h3>
			<p v-if="labels && !labels.field" class="text-sm text-muted-foreground" data-testid="knowledge-access-off">{{ t('off') }}</p>
			<p v-else-if="labels?.field" class="text-sm text-muted-foreground">{{ t('description', {field: labels.field}) }}</p>
		</div>

		<template v-if="labels?.field">
			<p v-if="labels.unassigned > 0" class="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive" data-testid="knowledge-access-unassigned">
				<TriangleAlert class="mt-0.5 h-4 w-4 shrink-0" />
				{{ t('unassigned', {count: String(labels.unassigned)}) }}
			</p>
			<p v-if="labels.error" class="text-sm text-muted-foreground" data-testid="knowledge-access-error">
				{{ t('store_error', {reason: store.getTranslation(`rag.store_error.${labels.error}`)}) }}
			</p>
			<p v-else-if="!labels.labels.some(label => label.found)" class="text-sm text-muted-foreground">{{ t('none_found') }}</p>

			<div class="grid gap-4 md:grid-cols-[minmax(0,16rem)_1fr]">
				<div class="space-y-2">
					<ul class="max-h-96 space-y-1 overflow-auto">
						<li v-for="label in labels.labels" :key="label.label">
							<button
								type="button"
								class="flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted"
								:class="{'bg-muted': selected === label.label}"
								:data-testid="`knowledge-access-label-${label.label}`"
								@click="select(label.label)"
							>
								<span class="truncate font-mono text-xs" :title="label.found ? label.label : t('not_found')" :class="{'italic text-muted-foreground': !label.found}">{{ label.label }}</span>
								<span class="shrink-0 text-xs" :class="label.holders ? 'text-muted-foreground' : 'text-destructive'">
									{{ label.holders ? t('holders', {count: String(label.holders)}) : t('nobody') }}
								</span>
							</button>
						</li>
					</ul>
					<form v-if="canManage" class="flex gap-2" @submit.prevent="add">
						<ShadInput v-model="newLabel" class="flex-1" maxlength="200" :placeholder="t('add_placeholder')" data-testid="knowledge-access-new" />
						<ShadButton type="submit" variant="outline" :disabled="!newLabel.trim()" data-testid="knowledge-access-add">{{ t('add') }}</ShadButton>
					</form>
				</div>

				<div>
					<p v-if="!selected" class="text-sm text-muted-foreground">{{ t('pick') }}</p>
					<SettingsAssignmentPicker
						v-else
						:key="selected"
						:model-value="data?.targets ?? []"
						:options="options"
						:kinds="data?.kinds ?? []"
						:roles="[]"
						:disabled="!data?.kinds.length"
						:description="t('holders_of', {label: selected})"
						:describe="describe"
						:empty-hint="t('empty_hint')"
						test-id="knowledge-access-holders"
						@update:model-value="change"
						@search="searchUsers"
					/>
				</div>
			</div>
		</template>
	</div>
</template>

<script setup lang="ts">
import {TriangleAlert} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AssignmentOption, AssignmentTarget} from '~/types/assignments';
import type {AccessLabelsResponse} from '~/types/generated/AccessLabelsResponse';

const props = defineProps<{id: string; canManage: boolean}>();
const store = useMainStore();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`knowledge.access.${key}`, params);
const {data, load, replace} = useAssignments();
const {teams, roles, users, searchUsers, load: loadOptions} = useAssignmentOptions();
const labels = ref<AccessLabelsResponse | null>(null);
const selected = ref<string | null>(null);
const newLabel = ref('');

const options = computed<AssignmentOption[]>(() => [...teams.value, ...roles.value, ...users.value]);

const describe = (target: AssignmentOption) => t(`effect.${target.kind}`, {name: target.label});

async function reload() {
	try {
		labels.value = await knowledgeAccessLabels(props.id);
	} catch (error: any) {
		store.toast(t('load_error'), {type: 'error', description: ragErrorMessage(error, '')});
	}
}

async function select(label: string) {
	selected.value = label;
	await load(knowledgeLabelAssignmentsPath(props.id, label));
}

async function add() {
	const label = newLabel.value.trim();
	newLabel.value = '';
	if (labels.value && !labels.value.labels.some(entry => entry.label === label)) {
		labels.value.labels.push({label, found: false, holders: 0});
	}
	await select(label);
}

async function change(targets: AssignmentTarget[]) {
	if (selected.value && (await replace(knowledgeLabelAssignmentsPath(props.id, selected.value), targets))) await reload();
}

onMounted(async () => {
	await Promise.all([reload(), loadOptions()]);
});
</script>
