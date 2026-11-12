<template>
	<div class="space-y-3" :data-testid="testId">
		<div v-if="description || canAdd" class="flex flex-wrap items-start justify-between gap-x-6 gap-y-2">
			<p v-if="description" class="max-w-prose flex-1 basis-64 text-sm text-muted-foreground">{{ description }}</p>
			<ShadPopover v-if="canAdd" v-model:open="open">
				<ShadPopoverTrigger as-child>
					<ShadButton variant="outline" size="sm" type="button" class="shrink-0 gap-2" :data-testid="`${testId}-add`">
						<Plus class="h-4 w-4" />
						{{ t('add') }}
					</ShadButton>
				</ShadPopoverTrigger>
				<ShadPopoverContent class="w-80 p-0" align="end">
					<div class="space-y-2 border-b border-border p-2">
						<ShadInput v-model="search" :placeholder="t('search')" class="h-8 text-sm" :data-testid="`${testId}-search`" />
						<div v-if="kinds.length > 1" class="flex flex-wrap gap-1">
							<button
								v-for="kind in [null, ...kinds]"
								:key="kind ?? 'all'"
								type="button"
								class="rounded-md px-2 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
								:class="filter === kind ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'"
								:aria-pressed="filter === kind"
								@click="filter = kind"
							>
								{{ kind ? t(`kind.${kind}`) : t('all') }}
							</button>
						</div>
					</div>
					<div class="max-h-64 overflow-y-auto p-1">
						<template v-for="group in optionGroups" :key="group.kind">
							<p v-if="kinds.length > 1 && !filter" class="px-2 pb-1 pt-2 text-xs font-medium text-muted-foreground">{{ t(`kind.${group.kind}`) }}</p>
							<button
								v-for="option in group.options"
								:key="keyOf(option)"
								type="button"
								class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-sm text-foreground transition-colors hover:bg-accent focus-visible:bg-accent focus-visible:outline-none disabled:pointer-events-none disabled:opacity-60"
								:aria-pressed="selected.has(keyOf(option))"
								:disabled="isFixed(selected.get(keyOf(option)))"
								:data-testid="`${testId}-option-${keyOf(option)}`"
								@click="toggle(option)"
							>
								<span class="flex h-4 w-4 shrink-0 items-center justify-center rounded-sm border" :class="selected.has(keyOf(option)) ? 'border-primary bg-primary text-primary-foreground' : 'border-input'">
									<Check v-if="selected.has(keyOf(option))" class="h-3 w-3" />
								</span>
								<component :is="ICONS[option.kind]" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
								<span class="min-w-0 flex-1 truncate">{{ labelOf(option) }}</span>
								<span v-if="isFixed(selected.get(keyOf(option)))" class="shrink-0 text-xs text-muted-foreground">{{ t(selected.get(keyOf(option))?.owner ? 'owner_badge' : 'inherited_badge') }}</span>
							</button>
						</template>
						<p v-if="!optionGroups.length" class="px-2 py-6 text-center text-sm text-muted-foreground">{{ t('no_results') }}</p>
					</div>
					<p v-if="note" class="border-t border-border px-3 py-2 text-xs text-muted-foreground">{{ note }}</p>
				</ShadPopoverContent>
			</ShadPopover>
		</div>

		<template v-if="single">
			<div role="radiogroup" class="divide-y divide-border overflow-hidden rounded-lg border border-border">
				<div v-for="group in choiceGroups" :key="group.kind" class="transition-colors" :class="activeKind === group.kind ? 'bg-accent/50' : ''">
					<button
						type="button"
						role="radio"
						class="flex w-full items-start gap-3 px-3 py-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring disabled:cursor-default"
						:class="[disabled || activeKind === group.kind ? '' : 'hover:bg-accent/30', disabled && activeKind !== group.kind ? 'opacity-60' : '']"
						:aria-checked="activeKind === group.kind"
						:disabled="disabled"
						:data-testid="group.options.length === 1 ? `${testId}-option-${keyOf(group.options[0]!)}` : `${testId}-kind-${group.kind}`"
						@click="pickGroup(group)"
					>
						<span class="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border bg-background" :class="activeKind === group.kind ? 'border-primary' : 'border-muted-foreground/50'">
							<span v-if="activeKind === group.kind" class="h-2 w-2 rounded-full bg-primary" />
						</span>
						<component :is="ICONS[group.kind]" class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
						<span class="min-w-0 flex-1">
							<span class="flex flex-wrap items-center gap-2">
								<span class="truncate text-sm font-medium text-foreground">{{ group.options.length === 1 ? labelOf(group.options[0]!) : t(`kind_one.${group.kind}`) }}</span>
								<span v-if="current?.kind === group.kind && activeKind !== group.kind" class="rounded-full border border-border px-1.5 py-0.5 text-xs text-muted-foreground">{{ t('current') }}</span>
							</span>
							<span v-if="describe" class="mt-0.5 block text-xs text-muted-foreground">{{ describe(representative(group)) }}</span>
						</span>
					</button>
					<div v-if="group.options.length > 1 && activeKind === group.kind" class="pb-3 pl-[4.25rem] pr-3">
						<ShadSelect :model-value="selectedKey(group)" :disabled="disabled" @update:model-value="key => chooseByKey(group, String(key))">
							<ShadSelectTrigger class="w-full max-w-sm bg-background" :data-testid="`${testId}-select`">
								<ShadSelectValue :placeholder="t('choose_one')" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="option in group.options" :key="keyOf(option)" :value="keyOf(option)" :data-testid="`${testId}-option-${keyOf(option)}`">{{ labelOf(option) }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
				</div>
			</div>
			<div v-if="pending" class="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border bg-muted/40 px-3 py-2" :data-testid="`${testId}-pending`">
				<p class="min-w-0 flex-1 basis-48 text-sm text-foreground">{{ t('move_confirm', {from: labelOf(current ?? pending), to: labelOf(pending)}) }}</p>
				<div class="flex shrink-0 gap-2">
					<ShadButton variant="ghost" size="sm" type="button" @click="reset">{{ t('cancel') }}</ShadButton>
					<ShadButton size="sm" type="button" :data-testid="`${testId}-confirm`" @click="confirmPending">{{ t('move') }}</ShadButton>
				</div>
			</div>
		</template>

		<template v-else>
			<div v-if="!modelValue.length" class="rounded-lg border border-dashed border-border px-4 py-6 text-center" :data-testid="`${testId}-empty`">
				<p class="text-sm font-medium text-foreground">{{ emptyTitle ?? t('none') }}</p>
				<p v-if="emptyHint" class="mx-auto mt-1 max-w-prose text-xs text-muted-foreground">{{ emptyHint }}</p>
			</div>
			<div v-else class="max-h-[28rem] overflow-y-auto rounded-lg border border-border">
				<section v-for="group in assignedGroups" :key="group.kind">
					<header v-if="kinds.length > 1" class="flex items-center justify-between border-b border-border bg-muted/40 px-3 py-1.5 text-xs font-medium text-muted-foreground">
						<span>{{ t(`kind.${group.kind}`) }}</span>
						<span class="tabular-nums">{{ group.targets.length }}</span>
					</header>
					<ul class="divide-y divide-border border-b border-border last:border-b-0">
						<li v-for="target in group.targets" :key="keyOf(target)" class="flex items-center gap-3 px-3 py-2.5" :data-testid="`${testId}-row`" :data-inherited="target.inherited">
							<span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted">
								<component :is="ICONS[target.kind]" class="h-4 w-4 text-muted-foreground" />
							</span>
							<div class="min-w-0 flex-1">
								<div class="flex items-center gap-2">
									<span class="truncate text-sm font-medium text-foreground" :data-testid="`${testId}-label`">{{ labelOf(target) }}</span>
									<span v-if="isFixed(target)" class="shrink-0 rounded-full border border-border px-1.5 py-0.5 text-xs text-muted-foreground">{{ t(target.owner ? 'owner_badge' : 'inherited_badge') }}</span>
								</div>
								<p class="text-xs text-muted-foreground">
									{{ target.inherited ? (inheritedHint ?? t('inherited')) : describe?.(target) }}
									<NuxtLink v-if="target.inherited && inheritedTo" :to="inheritedTo" class="text-primary hover:underline">{{ t('inherited_link') }}</NuxtLink>
								</p>
							</div>
							<ShadSelect v-if="roles.length && target.role && !disabled" :model-value="target.role" @update:model-value="role => setRole(target, role as AssignmentRole)">
								<ShadSelectTrigger class="h-8 w-auto shrink-0 gap-1.5 text-xs" :aria-label="`${t('role.label')}: ${labelOf(target)}`" :data-testid="`${testId}-role`">
									<ShadSelectValue />
								</ShadSelectTrigger>
								<ShadSelectContent align="end">
									<ShadSelectItem v-for="role in roles" :key="role" :value="role" :data-testid="`${testId}-role-${role}`">{{ t(`role.${role}`) }}</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
							<span v-else-if="target.role" class="shrink-0 text-xs text-muted-foreground" :data-testid="`${testId}-role`">{{ t(`role.${target.role}`) }}</span>
							<ShadButton
								v-if="!disabled && !isFixed(target)"
								variant="ghost"
								size="icon"
								type="button"
								class="h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive"
								:aria-label="`${t('remove')}: ${labelOf(target)}`"
								:data-testid="`${testId}-remove`"
								@click="toggle(target)"
							>
								<X class="h-4 w-4" />
							</ShadButton>
						</li>
					</ul>
				</section>
			</div>
		</template>

		<p v-if="note && (single || !canAdd)" class="flex items-start gap-1.5 text-xs text-muted-foreground">
			<Info class="mt-0.5 h-3.5 w-3.5 shrink-0" />
			<span class="max-w-prose">{{ note }}</span>
		</p>
	</div>
</template>

<script setup lang="ts">
import {Building2, Check, Globe, Info, KeyRound, Plus, Shield, User, X} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AssignmentKind, AssignmentOption, AssignmentRole, AssignmentTarget} from '~/types/assignments';

const props = withDefaults(
	defineProps<{
		modelValue: AssignmentTarget[];
		options: AssignmentOption[];
		kinds: AssignmentKind[];
		roles?: AssignmentRole[];
		single?: boolean;
		confirm?: boolean;
		disabled?: boolean;
		description?: string;
		describe?: (target: AssignmentOption) => string;
		inheritedHint?: string;
		inheritedTo?: string;
		emptyTitle?: string;
		emptyHint?: string;
		note?: string;
		testId?: string;
	}>(),
	{testId: 'assignments', roles: () => []},
);

const emit = defineEmits<{'update:modelValue': [targets: AssignmentTarget[]]}>();

const ICONS: Record<AssignmentKind, unknown> = {user: User, team: Building2, role: Shield, key: KeyRound, everyone: Globe};

const store = useMainStore();
const t = (key: string, args?: Record<string, string>) => store.getTranslation(`assignments.${key}`, args);

const open = ref(false);
const search = ref('');
const filter = ref<AssignmentKind | null>(null);
const pending = ref<AssignmentOption | null>(null);
const openKind = ref<AssignmentKind | null>(null);

const keyOf = (target: AssignmentOption) => (target.id ? `${target.kind}:${target.id}` : target.kind);
const labelOf = (target: AssignmentOption) => (target.kind === 'everyone' ? t('everyone') : (props.options.find(option => keyOf(option) === keyOf(target))?.label ?? target.label));
const matches = (option: AssignmentOption) => labelOf(option).toLowerCase().includes(search.value.trim().toLowerCase());

const isFixed = (target?: AssignmentTarget) => !!target && (target.inherited || target.owner);
const canAdd = computed(() => !props.single && !props.disabled);
const selected = computed(() => new Map(props.modelValue.map(target => [keyOf(target), target])));

const optionGroups = computed(() =>
	props.kinds
		.filter(kind => !filter.value || filter.value === kind)
		.map(kind => ({kind, options: props.options.filter(option => option.kind === kind && matches(option))}))
		.filter(group => group.options.length > 0),
);

const assignedGroups = computed(() =>
	(['everyone', 'role', 'team', 'user', 'key'] as AssignmentKind[])
		.map(kind => ({kind, targets: props.modelValue.filter(target => target.kind === kind).sort((a, b) => Number(a.inherited) - Number(b.inherited))}))
		.sort((a, b) => Number(b.targets.some(target => target.owner)) - Number(a.targets.some(target => target.owner)))
		.filter(group => group.targets.length > 0),
);

const KIND_ORDER: AssignmentKind[] = ['user', 'team', 'role', 'key', 'everyone'];
type ChoiceGroup = {kind: AssignmentKind; options: AssignmentOption[]};

const current = computed(() => props.modelValue[0] ?? null);
const activeKind = computed(() => pending.value?.kind ?? openKind.value ?? current.value?.kind ?? null);
const choiceGroups = computed<ChoiceGroup[]>(() => {
	const known = new Set(props.options.map(keyOf));
	const all: AssignmentOption[] = [...props.modelValue.filter(target => !known.has(keyOf(target))), ...props.options];
	return KIND_ORDER.map(kind => ({kind, options: all.filter(option => option.kind === kind)})).filter(group => group.options.length > 0);
});

const isCurrent = (choice: AssignmentOption) => selected.value.has(keyOf(choice));

function chosenIn(group: ChoiceGroup) {
	const chosen = pending.value ?? (openKind.value ? null : current.value);
	return chosen?.kind === group.kind ? chosen : null;
}

function representative(group: ChoiceGroup): AssignmentOption {
	return chosenIn(group) ?? (group.options.length === 1 ? group.options[0]! : {kind: group.kind, id: null, label: ''});
}

function reset() {
	pending.value = null;
	openKind.value = null;
}

watch(() => props.modelValue, reset);

function pickGroup(group: ChoiceGroup) {
	reset();
	if (group.options.length === 1) choose(group.options[0]!);
	else if (group.kind !== current.value?.kind) openKind.value = group.kind;
}

function selectedKey(group: ChoiceGroup) {
	const chosen = chosenIn(group);
	return chosen ? keyOf(chosen) : undefined;
}

function chooseByKey(group: ChoiceGroup, key: string) {
	const option = group.options.find(candidate => keyOf(candidate) === key);
	if (option) choose(option);
}

function asTarget(option: AssignmentOption): AssignmentTarget {
	return {kind: option.kind, id: option.id, label: option.label, inherited: false, owner: false, role: option.kind === 'everyone' ? null : (props.roles[0] ?? null)};
}

function choose(choice: AssignmentOption) {
	if (isCurrent(choice)) reset();
	else if (props.confirm) pending.value = choice;
	else emit('update:modelValue', [asTarget(choice)]);
}

function confirmPending() {
	if (pending.value) emit('update:modelValue', [asTarget(pending.value)]);
}

function setRole(target: AssignmentTarget, role: AssignmentRole) {
	emit(
		'update:modelValue',
		props.modelValue.map(current => (keyOf(current) === keyOf(target) ? {...current, role} : current)),
	);
}

function toggle(option: AssignmentOption) {
	const rest = props.modelValue.filter(current => keyOf(current) !== keyOf(option));
	emit('update:modelValue', rest.length === props.modelValue.length ? [...props.modelValue, asTarget(option)] : rest);
}
</script>
