<template>
	<div class="min-h-screen bg-background text-foreground selection:bg-primary/20 font-sans">
		<div class="container mx-auto">
			<div class="mb-6 lg:mb-8">
				<button
					@click="router.push('/')"
					class="group mb-4 flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
				>
					<ArrowLeft class="h-4 w-4 transition-transform group-hover:-translate-x-1" />
					<span>{{ store.getTranslation('developer.back') }}</span>
				</button>
				<h1 class="text-3xl font-bold tracking-tight text-foreground">{{ store.getTranslation('developer.title') }}</h1>
				<p class="text-base text-muted-foreground">{{ store.getTranslation('developer.description') }}</p>
			</div>

			<div class="grid w-full grid-cols-1 gap-4 lg:grid-cols-4 lg:gap-8">
				<aside class="w-full shrink-0 lg:w-64 lg:pr-2">
					<nav class="-mx-1 flex gap-1 overflow-x-auto px-1 pb-1 lg:mx-0 lg:block lg:space-y-1 lg:overflow-visible lg:px-0 lg:pb-0">
						<NuxtLink
							v-for="tab in tabs"
							:key="tab.id"
							:to="`/developer/${tab.id}`"
							class="flex shrink-0 items-center gap-3 whitespace-nowrap rounded-lg px-3 py-2 text-[15px] transition-all duration-200"
							:class="activeTab === tab.id ? 'bg-accent font-medium text-foreground' : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'"
						>
							<component :is="tab.icon" class="h-4 w-4" />
							<span>{{ store.getTranslation(tab.label) }}</span>
						</NuxtLink>
						<NuxtLink
							v-if="store.hasPermission('admin.developer.view')"
							to="/settings/gateway-admin"
							class="flex shrink-0 items-center gap-3 whitespace-nowrap rounded-lg px-3 py-2 text-[15px] text-muted-foreground transition-all duration-200 hover:bg-accent/50 hover:text-foreground"
						>
							<ShieldCheck class="h-4 w-4" />
							<span>{{ store.getTranslation('developer.nav.gateway_admin') }}</span>
						</NuxtLink>
						<NuxtLink
							v-if="store.isAdmin"
							to="/settings/admin-config"
							class="flex shrink-0 items-center gap-3 whitespace-nowrap rounded-lg px-3 py-2 text-[15px] text-muted-foreground transition-all duration-200 hover:bg-accent/50 hover:text-foreground lg:mt-4 lg:border-t lg:border-border lg:pt-4"
						>
							<ShieldCheck class="h-4 w-4" />
							<span>{{ store.getTranslation('developer.nav.admin') }}</span>
						</NuxtLink>
					</nav>
				</aside>

				<main class="min-w-0 lg:col-span-3">
					<div class="animate-in fade-in slide-in-from-bottom-2 duration-300">
						<NuxtPage />
					</div>
				</main>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {ArrowLeft, LayoutDashboard, KeyRound, BarChart3, ScrollText, Bot, FlaskConical, BookOpen, ShieldCheck} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

definePageMeta({
	layout: 'developer',
});

const store = useMainStore();
const route = useRoute();
const router = useRouter();

const tabs = [
	{id: 'overview', label: 'developer.nav.overview', icon: LayoutDashboard},
	{id: 'keys', label: 'developer.nav.keys', icon: KeyRound},
	{id: 'usage', label: 'developer.nav.usage', icon: BarChart3},
	{id: 'logs', label: 'developer.nav.logs', icon: ScrollText},
	{id: 'models', label: 'developer.nav.models', icon: Bot},
	{id: 'playground', label: 'developer.nav.playground', icon: FlaskConical},
	{id: 'docs', label: 'developer.nav.docs', icon: BookOpen},
];

const activeTab = computed(() => route.path.match(/\/developer\/([^/]+)/)?.[1] ?? 'overview');

onMounted(() => {
	if (route.path === '/developer' || route.path === '/developer/') {
		router.replace('/developer/overview');
	}
});
</script>
