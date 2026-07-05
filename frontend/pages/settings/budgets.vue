<template>
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between mb-6">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.budgets') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.budgets.description') }}</p>
			</div>
			<div class="flex items-center gap-2">
				<template v-if="canEdit && selected">
					<ShadButton variant="outline" size="sm" class="text-destructive hover:text-destructive" @click="deleteBudget">
						<Trash2 class="h-4 w-4" />
					</ShadButton>
					<ShadButton variant="default" size="sm" @click="saveBudget">
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</template>
				<ShadButton v-if="canEdit" variant="default" size="sm" class="gap-2" @click="createBudget">
					<Plus class="h-4 w-4" />
					<span>{{ store.getTranslation('settings.budgets.create') }}</span>
				</ShadButton>
			</div>
		</div>

		<div class="grid gap-4 lg:grid-cols-[19rem_1fr]">
			<!-- Budget list -->
			<div class="space-y-3">
				<div v-if="loading" class="flex items-center justify-center py-10 text-muted-foreground">
					<Loader2 class="h-5 w-5 animate-spin" />
				</div>
				<div v-else-if="budgetStore.budgets.length === 0" class="rounded-lg border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
					{{ store.getTranslation('settings.budgets.create') }}
				</div>
				<div v-else class="space-y-2">
					<button
						v-for="budget in budgetStore.budgets"
						:key="budget.id"
						type="button"
						class="w-full rounded-lg border px-3 py-2.5 text-left transition-colors"
						:class="selected?.id === budget.id ? 'border-primary/60 bg-accent/40' : 'border-border bg-card hover:border-primary/40 hover:bg-accent/10'"
						@click="selectBudget(budget)"
					>
						<div class="flex items-center justify-between gap-2">
							<span class="truncate font-medium text-foreground">{{ budget.name }}</span>
							<span class="shrink-0 rounded-full px-2 py-0.5 text-xs font-medium" :class="budget.on_exceed === 'block' ? 'bg-destructive/10 text-destructive' : budget.on_exceed === 'warn' ? 'bg-amber-500/10 text-amber-500' : 'bg-green-500/10 text-green-500'">
								{{ budget.on_exceed }}
							</span>
						</div>
						<div class="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
							<span class="font-medium text-foreground">{{ formatMoney(budget.amount) }}</span>
							<span>·</span>
							<span>{{ budget.kind }}</span>
							<span>·</span>
							<span>{{ budget.interval }}</span>
						</div>
					</button>
				</div>
			</div>

			<!-- Detail panel -->
			<div v-if="selected" class="space-y-4">
				<!-- Core settings -->
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="grid gap-4 md:grid-cols-2">
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.name') }}</ShadLabel>
							<ShadInput v-model="form.name" :disabled="!canEdit" />
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.amount') }}</ShadLabel>
							<ShadInput v-model="form.amount" :disabled="!canEdit" type="number" step="0.0001" />
						</div>
						<div class="space-y-1.5 md:col-span-2">
							<ShadLabel>{{ store.getTranslation('settings.budgets.notes') }}</ShadLabel>
							<ShadTextarea :model-value="form.description ?? ''" :disabled="!canEdit" rows="2" @update:model-value="form.description = $event || null" />
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.kind') }}</ShadLabel>
							<ShadSelect v-model="form.kind" :disabled="!canEdit">
								<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="pooled">pooled</ShadSelectItem>
									<ShadSelectItem value="per_user">per_user</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.interval') }}</ShadLabel>
							<ShadSelect v-model="form.interval" :disabled="!canEdit">
								<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="daily">daily</ShadSelectItem>
									<ShadSelectItem value="weekly">weekly</ShadSelectItem>
									<ShadSelectItem value="monthly">monthly</ShadSelectItem>
									<ShadSelectItem value="total">total</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.reset_strategy') }}</ShadLabel>
							<ShadSelect v-model="form.reset_strategy" :disabled="!canEdit">
								<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="calendar">calendar</ShadSelectItem>
									<ShadSelectItem value="rolling">rolling</ShadSelectItem>
									<ShadSelectItem value="anchored">anchored</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.on_exceed') }}</ShadLabel>
							<ShadSelect v-model="form.on_exceed" :disabled="!canEdit">
								<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="block">block</ShadSelectItem>
									<ShadSelectItem value="warn">warn</ShadSelectItem>
									<ShadSelectItem value="allow">allow</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
					</div>
					<div class="mt-4 flex items-center gap-2 border-t border-border pt-4">
						<ShadCheckbox v-model:checked="form.is_enabled" :disabled="!canEdit" />
						<ShadLabel>{{ store.getTranslation('settings.budgets.enabled') }}</ShadLabel>
					</div>
				</div>

				<!-- Assignments -->
				<div class="rounded-lg border border-border bg-card p-4">
					<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.budgets.assignments') }}</h3>

					<!-- Current assignments -->
					<div v-if="budgetStore.assignments.length" class="mb-3 space-y-2">
						<div
							v-for="a in budgetStore.assignments"
							:key="a.id"
							class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2 text-sm"
						>
							<div class="flex items-center gap-2 min-w-0">
								<component :is="a.team_id ? Building2 : User" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
								<span class="truncate font-medium">{{ a.team_name ?? a.user_label ?? '–' }}</span>
								<span class="shrink-0 text-xs text-muted-foreground">{{ a.team_id ? 'team' : 'user' }}</span>
							</div>
							<ShadButton v-if="canEdit" variant="ghost" size="icon" class="h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive" @click="removeAssignment(a)">
								<X class="h-4 w-4" />
							</ShadButton>
						</div>
					</div>
					<p v-else class="mb-3 text-sm text-muted-foreground">{{ store.getTranslation('settings.budgets.no_assignments') }}</p>

					<!-- Add assignment -->
					<div v-if="canEdit" class="grid gap-3 md:grid-cols-2">
						<!-- Team picker -->
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.assign_team') }}</ShadLabel>
							<Popover v-model:open="teamPickerOpen">
								<PopoverTrigger as-child>
									<ShadButton variant="outline" class="w-full justify-between font-normal" @click="teamPickerOpen = true">
										<span :class="selectedTeamId ? 'text-foreground' : 'text-muted-foreground'">
											{{ selectedTeamId ? (teams.find(t => t.id === selectedTeamId)?.name ?? selectedTeamId) : store.getTranslation('settings.budgets.pick_team') }}
										</span>
										<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 text-muted-foreground" />
									</ShadButton>
								</PopoverTrigger>
								<PopoverContent class="w-72 p-2" align="start">
									<ShadInput v-model="teamSearch" :placeholder="store.getTranslation('settings.teams.search')" class="mb-2 h-8 text-sm" />
									<div class="max-h-52 overflow-y-auto space-y-0.5">
										<button
											v-for="t in filteredTeams"
											:key="t.id"
											type="button"
											class="w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent text-left"
											:class="selectedTeamId === t.id ? 'bg-accent text-accent-foreground' : 'text-foreground'"
											@click="selectedTeamId = t.id; teamPickerOpen = false; teamSearch = ''"
										>
											<Check v-if="selectedTeamId === t.id" class="h-3.5 w-3.5 shrink-0" />
											<span v-else class="h-3.5 w-3.5 shrink-0" />
											<span class="truncate">{{ t.name }}</span>
										</button>
										<p v-if="filteredTeams.length === 0" class="px-2 py-4 text-center text-xs text-muted-foreground">{{ store.getTranslation('settings.teams.search') }}</p>
									</div>
								</PopoverContent>
							</Popover>
							<ShadButton :disabled="!selectedTeamId" size="sm" class="w-full" @click="assignTeam">
								{{ store.getTranslation('settings.budgets.assign_team') }}
							</ShadButton>
						</div>

						<!-- User picker -->
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.budgets.assign_user') }}</ShadLabel>
							<Popover v-model:open="userPickerOpen">
								<PopoverTrigger as-child>
									<ShadButton variant="outline" class="w-full justify-between font-normal" @click="userPickerOpen = true">
										<span :class="selectedUserId ? 'text-foreground' : 'text-muted-foreground'">
											{{ selectedUserId ? (users.find(u => u.id === selectedUserId)?.label ?? selectedUserId) : store.getTranslation('settings.budgets.pick_user') }}
										</span>
										<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 text-muted-foreground" />
									</ShadButton>
								</PopoverTrigger>
								<PopoverContent class="w-72 p-2" align="start">
									<ShadInput v-model="userSearch" :placeholder="store.getTranslation('settings.admin_users.search')" class="mb-2 h-8 text-sm" />
									<div class="max-h-52 overflow-y-auto space-y-0.5">
										<button
											v-for="u in filteredUsers"
											:key="u.id"
											type="button"
											class="w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent text-left"
											:class="selectedUserId === u.id ? 'bg-accent text-accent-foreground' : 'text-foreground'"
											@click="selectedUserId = u.id; userPickerOpen = false; userSearch = ''"
										>
											<Check v-if="selectedUserId === u.id" class="h-3.5 w-3.5 shrink-0" />
											<span v-else class="h-3.5 w-3.5 shrink-0" />
											<span class="truncate">{{ u.label }}</span>
										</button>
										<p v-if="filteredUsers.length === 0" class="px-2 py-4 text-center text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_users.search') }}</p>
									</div>
								</PopoverContent>
							</Popover>
							<ShadButton :disabled="!selectedUserId" size="sm" class="w-full" @click="assignUser">
								{{ store.getTranslation('settings.budgets.assign_user') }}
							</ShadButton>
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Plus, Trash2, Loader2, Check, ChevronsUpDown, Building2, User, X} from 'lucide-vue-next';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';
import {useMainStore} from '@/stores';
import {useBudgetStore} from '@/stores/budgetStore';
import type {Budget, BudgetAssignmentInfo, BudgetPayload} from '~/types/budgets';
import type {TeamList} from '~/types/chat';

interface UserEntry {
	id: string;
	label: string;
}

const store = useMainStore();
const budgetStore = useBudgetStore();
const {$customFetch} = useNuxtApp();

const loading = ref(false);
const selected = ref<Budget | null>(null);
const teams = ref<TeamList[]>([]);
const users = ref<UserEntry[]>([]);

const teamPickerOpen = ref(false);
const userPickerOpen = ref(false);
const teamSearch = ref('');
const userSearch = ref('');
const selectedTeamId = ref<string | null>(null);
const selectedUserId = ref<string | null>(null);

const form = reactive<BudgetPayload & {description: string | null}>({
	name: '',
	description: null,
	amount: '10.0000',
	kind: 'pooled',
	interval: 'monthly',
	reset_strategy: 'calendar',
	on_exceed: 'block',
	is_enabled: true,
});

const canEdit = computed(() => store.hasPermission('admin.budgets.edit'));

const filteredTeams = computed(() => {
	const q = teamSearch.value.toLowerCase();
	return q ? teams.value.filter(t => t.name.toLowerCase().includes(q)) : teams.value;
});

const filteredUsers = computed(() => {
	const q = userSearch.value.toLowerCase();
	return q ? users.value.filter(u => u.label.toLowerCase().includes(q)) : users.value;
});

function fillForm(budget: Budget) {
	form.name = budget.name;
	form.description = budget.description;
	form.amount = budget.amount;
	form.kind = budget.kind;
	form.interval = budget.interval;
	form.reset_strategy = budget.reset_strategy;
	form.on_exceed = budget.on_exceed;
	form.is_enabled = budget.is_enabled;
}

async function selectBudget(budget: Budget) {
	selected.value = budget;
	fillForm(budget);
	selectedTeamId.value = null;
	selectedUserId.value = null;
	await budgetStore.fetchAssignments(budget.id);
}

function formatMoney(value: string) {
	return `$${Number(value).toFixed(2)}`;
}

async function createBudget() {
	const budget = await budgetStore.createBudget({...form, name: form.name || store.getTranslation('settings.tabs.budgets')});
	await selectBudget(budget);
}

async function saveBudget() {
	if (!selected.value) return;
	const budget = await budgetStore.updateBudget(selected.value.id, {...form});
	await selectBudget(budget);
}

async function deleteBudget() {
	if (!selected.value) return;
	await budgetStore.deleteBudget(selected.value.id);
	selected.value = budgetStore.budgets[0] ?? null;
	if (selected.value) await selectBudget(selected.value);
}

async function assignTeam() {
	if (!selected.value || !selectedTeamId.value) return;
	await budgetStore.assignBudget(selected.value.id, {team_id: selectedTeamId.value});
	selectedTeamId.value = null;
}

async function assignUser() {
	if (!selected.value || !selectedUserId.value) return;
	await budgetStore.assignBudget(selected.value.id, {user_id: selectedUserId.value});
	selectedUserId.value = null;
}

async function removeAssignment(a: BudgetAssignmentInfo) {
	if (!selected.value) return;
	await budgetStore.removeAssignment(selected.value.id, a.id);
}

async function loadTeamsAndUsers() {
	const [teamsRes, usersRes] = await Promise.all([
		$customFetch<{items: TeamList[]; has_more: boolean}>('/api/v1/admin/teams', {params: {size: 200}}),
		$customFetch<{users: {id: string; username: string; email: string}[]; total: number}>('/api/v1/admin/users', {params: {per_page: 200}}),
	]);
	teams.value = teamsRes?.items ?? [];
	users.value = (usersRes?.users ?? []).map(u => ({id: u.id, label: u.username || u.email}));
}

onMounted(async () => {
	loading.value = true;
	try {
		await Promise.all([budgetStore.fetchBudgets(), loadTeamsAndUsers()]);
		if (budgetStore.budgets[0]) await selectBudget(budgetStore.budgets[0]);
	} finally {
		loading.value = false;
	}
});
</script>
