<template>
	<div class="space-y-2" :data-testid="testId">
		<div v-if="modelValue.length" class="flex flex-wrap gap-1.5">
			<span
				v-for="target in modelValue"
				:key="keyOf(target)"
				class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/40 px-2 py-1 text-sm text-foreground"
				:class="{'opacity-60': target.inherited}"
				:title="target.inherited ? t('inherited') : undefined"
				:data-testid="`${testId}-chip`"
			>
				<component :is="ICONS[target.kind]" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
				<span class="max-w-48 truncate">{{ labelOf(target) }}</span>
				<button v-if="!disabled && !single && !target.inherited" type="button" class="text-muted-foreground hover:text-destructive" :aria-label="t('remove')" :data-testid="`${testId}-remove`" @click="toggle(target)">
					<X class="h-3.5 w-3.5" />
				</button>
			</span>
		</div>
		<p v-else class="text-sm text-muted-foreground">{{ t('none') }}</p>

		<ShadPopover v-if="!disabled" v-model:open="open">
			<ShadPopoverTrigger as-child>
				<ShadButton variant="outline" size="sm" type="button" class="gap-2" :data-testid="`${testId}-add`">
					<Plus class="h-4 w-4" />
					{{ t(single ? 'change' : 'add') }}
				</ShadButton>
			</ShadPopoverTrigger>
			<ShadPopoverContent class="w-72 p-2" align="start">
				<ShadInput v-model="search" :placeholder="t('search')" class="mb-2 h-8 text-sm" />
				<div class="max-h-60 space-y-0.5 overflow-y-auto">
					<template v-for="group in groups" :key="group.kind">
						<p v-if="kinds.length > 1" class="px-2 pt-1 text-xs font-medium text-muted-foreground">{{ t(`kind.${group.kind}`) }}</p>
						<button
							v-for="option in group.options"
							:key="keyOf(option)"
							type="button"
							class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-foreground hover:bg-accent disabled:opacity-60"
							:disabled="selected.get(keyOf(option))?.inherited"
							:data-testid="`${testId}-option-${keyOf(option)}`"
							@click="toggle(option)"
						>
							<Check v-if="selected.has(keyOf(option))" class="h-3.5 w-3.5 shrink-0" />
							<span v-else class="h-3.5 w-3.5 shrink-0" />
							<span class="truncate">{{ labelOf(option) }}</span>
						</button>
					</template>
					<p v-if="!groups.length" class="px-2 py-3 text-center text-sm text-muted-foreground">{{ t('no_results') }}</p>
				</div>
			</ShadPopoverContent>
		</ShadPopover>
	</div>
</template>

<script setup lang="ts">
import {Building2, Check, Globe, KeyRound, Plus, Shield, User, X} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AssignmentKind, AssignmentOption, AssignmentTarget} from '~/types/assignments';

const props = withDefaults(
	defineProps<{
		modelValue: AssignmentTarget[];
		options: AssignmentOption[];
		kinds: AssignmentKind[];
		single?: boolean;
		disabled?: boolean;
		testId?: string;
	}>(),
	{testId: 'assignments'},
);

const emit = defineEmits<{'update:modelValue': [targets: AssignmentTarget[]]}>();

const ICONS: Record<AssignmentKind, unknown> = {user: User, team: Building2, role: Shield, key: KeyRound, everyone: Globe};

const store = useMainStore();
const t = (key: string) => store.getTranslation(`assignments.${key}`);

const open = ref(false);
const search = ref('');

const keyOf = (target: AssignmentOption) => (target.id ? `${target.kind}:${target.id}` : target.kind);
const labelOf = (target: AssignmentOption) => (target.kind === 'everyone' ? t('everyone') : target.label);

const selected = computed(() => new Map(props.modelValue.map(target => [keyOf(target), target])));

const groups = computed(() => {
	const query = search.value.trim().toLowerCase();
	return props.kinds
		.map(kind => ({kind, options: props.options.filter(option => option.kind === kind && labelOf(option).toLowerCase().includes(query))}))
		.filter(group => group.options.length > 0);
});

function toggle(option: AssignmentOption) {
	const target: AssignmentTarget = {kind: option.kind, id: option.id, label: option.label, inherited: false};
	if (props.single) {
		open.value = false;
		if (!selected.value.has(keyOf(option))) emit('update:modelValue', [target]);
		return;
	}
	const rest = props.modelValue.filter(current => keyOf(current) !== keyOf(option));
	emit('update:modelValue', rest.length === props.modelValue.length ? [...props.modelValue, target] : rest);
}
</script>
