<template>
	<div class="min-h-screen bg-background text-foreground selection:bg-primary/20 font-sans">
		<div class="container mx-auto">
			<div class="mb-8">
				<button
					@click="goBack"
					class="group flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors mb-8 px-3 py-2 rounded-lg"
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

			<div class="grid w-full grid-cols-1 gap-8 lg:grid-cols-4">
				<aside class="w-full shrink-0 lg:w-64 lg:pr-2">
					<nav class="space-y-1">
						<NuxtLink
							v-for="tab in visibleTabs"
							:key="tab.id"
							:to="`/settings/${tab.id}`"
							class="flex items-center gap-3 px-3 py-2 rounded-lg text-[15px] transition-all duration-200"
							:class="[activeTab === tab.id ? 'text-foreground font-medium bg-accent' : 'text-muted-foreground hover:text-foreground hover:bg-accent/50']"
						>
							<component :is="tab.icon" class="h-4 w-4" :class="activeTab === tab.id ? 'text-foreground' : 'text-muted-foreground'" />
							<span>{{ store.getTranslation(tab.label) }}</span>
						</NuxtLink>
					</nav>
				</aside>

				<main class="col-span-3 flex-1">
					<div class="animate-in fade-in slide-in-from-bottom-2 duration-300">
						<NuxtPage />
					</div>
				</main>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {User, ArrowLeft, Cpu} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

definePageMeta({
	layout: 'settings',
});

const store = useMainStore();
const route = useRoute();
const router = useRouter();

const tabs = [
	{id: 'profile', label: 'settings.tabs.profile', icon: User, permission: 'settings.profile.view'},
	{id: 'providers', label: 'settings.tabs.providers', icon: Cpu, permission: 'admin.providers.view'},
	{id: 'tools', label: 'settings.tabs.tools', icon: Cpu, permission: 'admin.tools.view'},
	{id: 'appearance', label: 'settings.tabs.appearance', icon: Cpu, permission: 'settings.appearance.view'},
];

const visibleTabs = computed(() => tabs.filter(tab => store.hasPermission(tab.permission)));

const activeTab = computed(() => {
	const path = route.path;
	const match = path.match(/\/settings\/(.+)/);
	return match ? match[1] : 'profile';
});

function goBack() {
	router.push('/');
}

onMounted(() => {
	if (route.path === '/settings' || route.path === '/settings/') {
		router.replace('/settings/profile');
	}
});
</script>
