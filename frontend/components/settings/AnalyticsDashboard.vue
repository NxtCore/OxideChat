<template>
	<div class="w-full">
		<!-- Header + date filter -->
		<div class="mb-6 flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ title }}</h2>
				<p class="text-sm text-muted-foreground">{{ description }}</p>
			</div>
			<div class="flex flex-col gap-2 sm:flex-row sm:items-start">
				<Popover v-if="isAdmin" v-model:open="userPickerOpen">
					<PopoverTrigger as-child>
						<ShadButton variant="outline" class="w-full justify-between font-normal sm:w-48" @click="userPickerOpen = true">
							<span :class="selectedUserId ? 'text-foreground' : 'text-muted-foreground'">
								{{ selectedUserId ? (userList.find(u => u.id === selectedUserId)?.label ?? selectedUserId) : translateOr('settings.analytics.all_users', 'All users') }}
							</span>
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 text-muted-foreground" />
						</ShadButton>
					</PopoverTrigger>
					<PopoverContent class="w-56 p-2" align="start">
						<ShadInput v-model="userSearch" :placeholder="translateOr('settings.admin_users.search', 'Search users...')" class="mb-2 h-8 text-sm" />
						<div class="max-h-52 overflow-y-auto space-y-0.5">
							<button
								type="button"
								class="w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent text-left"
								:class="!selectedUserId ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'"
								@click="selectedUserId = null; userPickerOpen = false; userSearch = ''"
							>
								<Check v-if="!selectedUserId" class="h-3.5 w-3.5 shrink-0" />
								<span v-else class="h-3.5 w-3.5 shrink-0" />
								<span>{{ translateOr('settings.analytics.all_users', 'All users') }}</span>
							</button>
							<button
								v-for="u in filteredUserList"
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
							<p v-if="filteredUserList.length === 0" class="px-2 py-4 text-center text-xs text-muted-foreground">{{ translateOr('settings.admin_users.no_users', 'No users found') }}</p>
						</div>
					</PopoverContent>
				</Popover>
				<SettingsAnalyticsDateFilter @change="onDateChange" />
			</div>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-20 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<template v-else>
			<!-- Tab navigation -->
			<div class="mb-6 flex items-center gap-1 border-b border-border">
				<button
					v-for="tab in tabs"
					:key="tab.id"
					class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px"
					:class="activeTab === tab.id
						? 'text-foreground border-primary'
						: 'text-muted-foreground border-transparent hover:text-foreground hover:border-border'"
					@click="activeTab = tab.id"
				>
					{{ store.getTranslation(tab.i18n) }}
				</button>
			</div>

			<KeepAlive>
				<SettingsAnalyticsOverviewTab
					v-if="activeTab === 'overview'"
					:is-admin="isAdmin"
					:by-model="analytics.byModel"
					:by-day="analytics.byDay"
					:by-user="byUser"
					:by-api-key="byApiKey"
					:stacked="stacked"
					:total-cost="totalCost"
					:day-count="dayCount"
				/>
				<SettingsAnalyticsTrendsTab
					v-else-if="activeTab === 'trends'"
					:is-admin="isAdmin"
					:by-model="analytics.byModel"
					:by-day-model="analytics.byDayModel"
					:by-user="byUser"
					:by-api-key="byApiKey"
					:stacked="stacked"
					:total-cost="totalCost"
				/>
				<SettingsAnalyticsExploreTab v-else :by-model="analytics.byModel" :by-day-model="analytics.byDayModel" :by-user="byUser" />
			</KeepAlive>
		</template>
	</div>
</template>

<script setup lang="ts">
import {Loader2, ChevronsUpDown, Check} from 'lucide-vue-next';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';
import {useMainStore} from '@/stores';
import {useBudgetStore} from '@/stores/budgetStore';

const props = defineProps<{
	isAdmin: boolean;
	userId?: string;
}>();

interface UserEntry {
	id: string;
	label: string;
}

const store = useMainStore();
const budgetStore = useBudgetStore();
const loading = ref(false);
const from = ref('');
const to = ref('');
const selectedUserId = ref<string | null>(props.userId ?? null);
const userList = ref<UserEntry[]>([]);
const userPickerOpen = ref(false);
const userSearch = ref('');
const activeTab = ref('overview');

const filteredUserList = computed(() => {
	const q = userSearch.value.trim().toLowerCase();
	return q ? userList.value.filter(u => u.label.toLowerCase().includes(q)) : userList.value;
});

const title = computed(() => {
	if (props.isAdmin && selectedUserId.value) {
		const user = userList.value.find(u => u.id === selectedUserId.value);
		return user?.label ?? store.getTranslation('settings.tabs.analytics');
	}
	return props.isAdmin
		? store.getTranslation('settings.tabs.analytics')
		: store.getTranslation('settings.analytics.my_usage');
});

const description = computed(() => store.getTranslation('settings.analytics.description'));

const tabs = [
	{id: 'overview', i18n: 'settings.analytics.tab_overview'},
	{id: 'trends', i18n: 'settings.analytics.tab_trends'},
	{id: 'explore', i18n: 'settings.analytics.tab_explore'},
];

function onDateChange(range: {from: string; to: string; label: string}) {
	from.value = range.from;
	to.value = range.to;
	load();
}

const analytics = computed(() => {
	if (!props.isAdmin) return budgetStore.myAnalytics;
	return selectedUserId.value ? budgetStore.userAnalytics : budgetStore.analytics;
});
const allUsers = computed(() => props.isAdmin && !selectedUserId.value);
const byUser = computed(() => {
	if (!allUsers.value) return [];
	return budgetStore.analytics.byUser.map(row => (row.id === null ? {...row, label: store.getTranslation('settings.analytics.service_accounts')} : row));
});
const byApiKey = computed(() => (allUsers.value ? budgetStore.analytics.byApiKey : []));

const totalCost = computed(() => analytics.value.byModel.reduce((s, r) => s + Number(r.cost_total), 0));
const stacked = computed(() => stackedModelSeries(analytics.value.byModel, analytics.value.byDayModel));

const dayCount = computed(() => {
	if (!from.value || !to.value) return 30;
	const d = (new Date(to.value).getTime() - new Date(from.value).getTime()) / 86_400_000;
	return Math.max(1, Math.round(d));
});

async function loadUsers() {
	const res = await adminUsers({size: 100});
	userList.value = (res?.items ?? []).map(u => ({id: u.id, label: u.username || u.email}));
}

async function load() {
	loading.value = true;
	try {
		const params = {
			from: from.value ? `${from.value}T00:00:00Z` : undefined,
			to: to.value ? `${to.value}T23:59:59Z` : undefined,
		};
		if (props.isAdmin && selectedUserId.value) {
			await budgetStore.fetchUserAnalytics(selectedUserId.value, params);
		} else if (props.isAdmin) {
			await budgetStore.fetchAllAnalytics(params);
		} else {
			await budgetStore.fetchMyAnalytics(params);
		}
	} catch {
		store.toast(store.getTranslation('common.error'), {type: 'error'});
	} finally {
		loading.value = false;
	}
}

watch(() => props.userId, (v) => {
	selectedUserId.value = v ?? null;
});

watch(selectedUserId, () => {
	load();
});

onMounted(() => {
	if (props.isAdmin) loadUsers();
});
</script>
