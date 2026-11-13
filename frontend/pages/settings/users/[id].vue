<template>
	<SettingsPage :title="overview?.profile.username ?? t(loading ? 'loading' : 'not_found_title')" :description="overview?.profile.email" :form="settingsForm">
		<template #leading>
			<ShadButton variant="ghost" size="icon" class="shrink-0" :aria-label="t('back')" data-testid="user-back" @click="router.push('/settings/users')">
				<ArrowLeft class="h-4 w-4" />
			</ShadButton>
			<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary text-sm font-semibold text-primary-foreground" aria-hidden="true">
				<template v-if="overview">{{ overview.profile.username.slice(0, 2).toUpperCase() }}</template>
				<User v-else class="h-5 w-5" />
			</div>
		</template>
		<template v-if="overview" #actions>
			<ShadButton v-if="store.hasPermission(ADMIN_ANALYTICS_VIEW)" as-child variant="outline">
				<NuxtLink :to="`/settings/analytics?user_id=${userId}`" data-testid="user-analytics">
					<BarChart2 class="h-4 w-4" />
					{{ store.getTranslation('settings.admin_users.view_analytics') }}
				</NuxtLink>
			</ShadButton>
			<ShadButton v-if="overview.can_edit && !isSelf" variant="outline" data-testid="user-delete" @click="deleteOpen = true">
				<Trash2 class="h-4 w-4" />
				{{ store.getTranslation('settings.admin_users.delete_user') }}
			</ShadButton>
		</template>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="!overview" class="flex items-center justify-center py-12 text-muted-foreground">
			<p data-testid="user-not-found">{{ t('not_found') }}</p>
		</div>

		<template v-else>
			<ShadTabs v-model="currentTab" class="mb-5 w-full">
				<div class="overflow-x-auto">
					<ShadTabsList>
						<ShadTabsTrigger v-for="tab in tabs" :key="tab" :value="tab" :data-testid="`user-tab-${tab}`">{{ t(`tabs.${tab}`) }}</ShadTabsTrigger>
					</ShadTabsList>
				</div>
			</ShadTabs>

			<SettingsUsersProfileTab v-show="currentTab === 'profile'" :overview="overview" :form="form" @changed="load" />
			<SettingsUsersRolesTab v-show="currentTab === 'roles'" :overview="overview" :targets="targetsOf('role')" :editable="canAssign('role')" @replace="targets => replaceKind('role', targets)" />
			<SettingsUsersTeamsTab
				v-show="currentTab === 'teams'"
				:overview="overview"
				:targets="targetsOf('team')"
				:editable="canAssign('team')"
				:active="currentTab === 'teams'"
				@replace="targets => replaceKind('team', targets)"
			/>
			<SettingsUsersModelsTab v-if="overview.model_access" v-show="currentTab === 'models'" :access="overview.model_access" />
			<SettingsUsersPresetsTab v-if="overview.presets" v-show="currentTab === 'presets'" :presets="overview.presets" :user-id="userId" :teams="overview.teams" />
			<SettingsUsersBudgetsTab v-if="overview.budgets" v-show="currentTab === 'budgets'" :budgets="overview.budgets" :teams="overview.teams" />
			<SettingsUsersKeysTab v-if="overview.keys" v-show="currentTab === 'keys'" :keys="overview.keys" />
		</template>

		<ShadDialog v-model:open="deleteOpen">
			<ShadDialogContent class="sm:max-w-[420px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('settings.admin_users.delete_user') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('settings.admin_users.delete_confirm', {username: overview?.profile.username ?? ''}) }}</ShadDialogDescription>
				</ShadDialogHeader>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="deleteOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton variant="destructive" :disabled="deleting" data-testid="user-delete-confirm" @click="remove">
						<Loader2 v-if="deleting" class="h-4 w-4 animate-spin" />
						{{ store.getTranslation('common.delete') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</SettingsPage>
</template>

<script setup lang="ts">
import {ArrowLeft, BarChart2, Loader2, Trash2, User} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AssignmentKind, AssignmentTarget} from '~/types/assignments';
import {ADMIN_ANALYTICS_VIEW} from '~/types/generated/permissions';
import type {UserOverviewResponse} from '~/types/users';

const TABS = ['profile', 'roles', 'teams', 'models', 'presets', 'budgets', 'keys'] as const;
type UserTab = (typeof TABS)[number];

const store = useMainStore();
const route = useRoute();
const router = useRouter();

const t = (key: string) => store.getTranslation(`settings.user_detail.${key}`);

const userId = route.params.id as string;
const assignmentsPath = `/api/v1/admin/users/${userId}/assignments`;

const overview = ref<UserOverviewResponse | null>(null);
const loading = ref(true);
const deleteOpen = ref(false);
const deleting = ref(false);
const form = reactive({email: '', username: ''});
const {data: assignments, replace} = useAssignments();

const queryTab = route.query.tab as UserTab | undefined;
const currentTab = ref<UserTab>(queryTab && TABS.includes(queryTab) ? queryTab : 'profile');

const isSelf = computed(() => userId === store.auth.user?.id);
const tabs = computed(() => {
	const sections: Partial<Record<UserTab, unknown>> = {
		models: overview.value?.model_access,
		presets: overview.value?.presets,
		budgets: overview.value?.budgets,
		keys: overview.value?.keys,
	};
	return TABS.filter(tab => !(tab in sections) || sections[tab]);
});

const settingsForm = useSettingsForm({
	value: () => form,
	restore: saved => Object.assign(form, saved),
	savedKey: 'settings.admin_users.edit_success',
	errorKey: 'settings.admin_users.edit_error',
	canEdit: () => overview.value?.can_edit ?? false,
	save: async () => {
		await updateAdminUser(userId, {email: form.email, username: form.username});
		await load();
	},
});

const targetsOf = (kind: AssignmentKind) => assignments.value?.targets.filter(target => target.kind === kind) ?? [];
const canAssign = (kind: AssignmentKind) => assignments.value?.kinds.includes(kind) ?? false;

async function load() {
	try {
		const loaded = await adminUserOverview(userId);
		const keepEdits = settingsForm.isDirty && !settingsForm.saving;
		overview.value = loaded;
		assignments.value = loaded.assignments;
		if (!tabs.value.includes(currentTab.value)) currentTab.value = 'profile';
		if (!keepEdits) {
			Object.assign(form, {email: loaded.profile.email, username: loaded.profile.username});
			settingsForm.markSaved();
		}
	} catch (e: any) {
		if (e?.status !== 404) store.toast(t('load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
}

async function replaceKind(kind: AssignmentKind, targets: AssignmentTarget[]) {
	const others = assignments.value?.targets.filter(target => target.kind !== kind) ?? [];
	if (await replace(assignmentsPath, [...others, ...targets])) await load();
}

async function remove() {
	deleting.value = true;
	try {
		await deleteAdminUser(userId);
		store.toast(store.getTranslation('settings.admin_users.delete_success'), {type: 'success'});
		deleteOpen.value = false;
		await router.push('/settings/users');
	} catch (e: any) {
		store.toast(store.getTranslation('settings.admin_users.delete_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		deleting.value = false;
	}
}

watch(currentTab, tab => router.replace({query: tab === 'profile' ? {} : {tab}}));

onMounted(load);
</script>
