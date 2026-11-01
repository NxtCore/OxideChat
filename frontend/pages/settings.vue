<template>
	<div class="min-h-screen bg-background text-foreground selection:bg-primary/20 font-sans lg:flex lg:h-[calc(100dvh-3rem)] lg:min-h-0 lg:flex-col">
		<div class="container mx-auto lg:flex lg:min-h-0 lg:flex-1 lg:flex-col">
			<div class="mb-8 lg:shrink-0">
				<button
					@click="goBack"
					class="group flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors mb-4 px-3 py-2 rounded-lg"
				>
					<ArrowLeft class="h-4 w-4 transition-transform group-hover:-translate-x-1" />
					<span>{{ store.getTranslation('settings.back') }}</span>
				</button>

				<h1 class="text-3xl font-bold tracking-tight text-foreground">
					{{ store.getTranslation('settings.title') }}
				</h1>
				<p class="text-muted-foreground text-base">
					{{ store.getTranslation('settings.description') }}
				</p>
			</div>

			<div class="grid w-full grid-cols-1 gap-8 lg:min-h-0 lg:flex-1 lg:grid-cols-4 lg:grid-rows-[minmax(0,1fr)]">
				<aside class="w-full shrink-0 lg:w-64 lg:overflow-y-auto lg:pr-2">
					<nav class="space-y-5">
						<div v-for="group in visibleGroups" :key="group.id" class="space-y-1" :data-testid="`settings-nav-${group.id}`">
							<p class="px-3 pb-1 text-xs font-medium text-muted-foreground">{{ store.getTranslation(group.label) }}</p>
							<NuxtLink
								v-for="tab in group.tabs"
								:key="tab.id"
								:to="`/settings/${tab.id}`"
								class="flex items-center gap-3 px-3 py-2 rounded-lg text-[15px] transition-all duration-200"
								:class="[activeTab === tab.id ? 'text-foreground font-medium bg-accent' : 'text-muted-foreground hover:text-foreground hover:bg-accent/50']"
							>
								<component :is="tab.icon" class="h-4 w-4" :class="activeTab === tab.id ? 'text-foreground' : 'text-muted-foreground'" />
								<span>{{ store.getTranslation(tab.label) }}</span>
							</NuxtLink>
						</div>
					</nav>
				</aside>

				<main class="col-span-3 min-w-0 flex-1 lg:overflow-y-auto">
					<div class="flex flex-col animate-in fade-in slide-in-from-bottom-2 duration-300 lg:min-h-full">
						<NuxtPage />
					</div>
				</main>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {ADMIN_ANALYTICS_VIEW, ADMIN_BUDGETS_VIEW, ADMIN_CONFIG_VIEW, ADMIN_DEVELOPER_VIEW, ADMIN_I18N_VIEW, ADMIN_PROVIDERS_VIEW, ADMIN_TEAMS_VIEW, ADMIN_TOOLS_VIEW, ADMIN_USERS_VIEW, SETTINGS_APPEARANCE_EDIT, SETTINGS_SESSIONS_VIEW} from '~/types/generated/permissions';
import {Activity, User, ArrowLeft, Cpu, Bot, Package, Palette, Users, Network, CreditCard, BarChart3, SlidersHorizontal, ShieldCheck, Languages, MonitorSmartphone} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

definePageMeta({
	layout: 'settings',
});

const store = useMainStore();
const route = useRoute();
const router = useRouter();

const groups = [
	{
		id: 'account',
		label: 'settings.groups.account',
		tabs: [
			{id: 'profile', label: 'settings.tabs.profile', icon: User, permission: 'settings.profile.view'},
			{id: 'usage', label: 'settings.analytics.my_usage', icon: Activity, permission: 'settings.profile.view'},
			{id: 'appearance', label: 'settings.tabs.appearance', icon: Palette, permission: SETTINGS_APPEARANCE_EDIT},
			{id: 'sessions', label: 'settings.tabs.sessions', icon: MonitorSmartphone, permission: SETTINGS_SESSIONS_VIEW},
		],
	},
	{
		id: 'ai',
		label: 'settings.groups.ai',
		tabs: [
			{id: 'providers', label: 'settings.tabs.providers', icon: Cpu, permission: ADMIN_PROVIDERS_VIEW},
			{id: 'models', label: 'settings.tabs.models', icon: Bot, permission: ADMIN_PROVIDERS_VIEW},
			{id: 'tools', label: 'settings.tabs.tools', icon: Package, permission: ADMIN_TOOLS_VIEW},
		],
	},
	{
		id: 'access',
		label: 'settings.groups.access',
		tabs: [
			{id: 'users', label: 'settings.tabs.admin_users', icon: Users, permission: ADMIN_USERS_VIEW},
			{id: 'teams', label: 'settings.tabs.teams', icon: Network, permission: ADMIN_TEAMS_VIEW},
			{id: 'budgets', label: 'settings.tabs.budgets', icon: CreditCard, permission: ADMIN_BUDGETS_VIEW},
		],
	},
	{
		id: 'operations',
		label: 'settings.groups.operations',
		tabs: [
			{id: 'analytics', label: 'settings.tabs.analytics', icon: BarChart3, permission: ADMIN_ANALYTICS_VIEW},
			{id: 'gateway-admin', label: 'settings.tabs.gateway_admin', icon: ShieldCheck, permission: ADMIN_DEVELOPER_VIEW},
			{id: 'translations', label: 'settings.tabs.translations', icon: Languages, permission: ADMIN_I18N_VIEW},
			{id: 'admin-config', label: 'settings.tabs.admin_config', icon: SlidersHorizontal, permission: ADMIN_CONFIG_VIEW},
		],
	},
];

const visibleGroups = computed(() =>
	groups.map(group => ({...group, tabs: group.tabs.filter(tab => store.hasPermission(tab.permission))})).filter(group => group.tabs.length > 0)
);

const activeTab = computed(() => {
	const path = route.path;
	const match = path.match(/\/settings\/([^\/]+)/);
	return match ? match[1] : 'profile';
});

function goBack() {
	const returnPath = store.settingsReturnPath;
	store.settingsReturnPath = '/';
	router.push(returnPath === '/' || returnPath.startsWith('/chats/') ? returnPath : '/');
}

onMounted(() => {
	if (route.path === '/settings' || route.path === '/settings/') {
		router.replace('/settings/profile');
	}
});
</script>
