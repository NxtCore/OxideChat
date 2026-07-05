<template>
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto">
		<ShadTabs v-model="activeProfileTab" class="w-full">
			<ShadTabsList class="mb-4 grid w-full grid-cols-2 sm:w-72">
				<ShadTabsTrigger value="profile">{{ store.getTranslation('settings.tabs.profile') }}</ShadTabsTrigger>
				<ShadTabsTrigger value="analytics">{{ store.getTranslation('settings.analytics.my_usage') }}</ShadTabsTrigger>
			</ShadTabsList>

			<ShadTabsContent value="profile" class="mt-0">
				<div class="rounded-lg border border-border bg-card p-6">
					<div class="mb-4">
						<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.profile.title') }}</h2>
						<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.profile.description') }}</p>
					</div>

					<div class="flex items-start gap-6 mb-6">
						<div class="h-20 w-20 rounded-lg bg-primary flex items-center justify-center">
							<span class="text-2xl font-semibold text-primary-foreground">{{ userInitials }}</span>
						</div>
					</div>

					<div class="flex items-center justify-between py-4 border-t border-border">
						<div>
							<span class="text-sm text-muted-foreground">{{ store.getTranslation('settings.profile.name') }}</span>
							<p class="text-foreground font-medium">{{ store.auth.user?.username }}</p>
						</div>
						<button @click="store.copyToClipboard(store.auth.user?.username as string)" class="text-muted-foreground hover:text-foreground transition-colors">
							<Copy class="h-4 w-4" />
						</button>
					</div>

					<div class="flex items-center justify-between py-4 border-t border-border">
						<div>
							<span class="text-sm text-muted-foreground">{{ store.getTranslation('settings.profile.email') }}</span>
							<p class="text-foreground font-medium">{{ store.auth.user?.email }}</p>
						</div>
						<button @click="store.copyToClipboard(store.auth.user?.email as string)" class="text-muted-foreground hover:text-foreground transition-colors">
							<Copy class="h-4 w-4" />
						</button>
					</div>

					<div class="flex items-center justify-between py-4 border-t border-border">
						<div>
							<span class="text-sm text-muted-foreground">{{ store.getTranslation('settings.profile.user_id') }}</span>
							<p class="text-foreground font-mono text-sm">{{ store.auth.user?.id }}</p>
						</div>
						<button @click="store.copyToClipboard(store.auth.user?.id as string)" class="text-muted-foreground hover:text-foreground transition-colors">
							<Copy class="h-4 w-4" />
						</button>
					</div>
				</div>
			</ShadTabsContent>

			<ShadTabsContent value="analytics" class="mt-0">
				<SettingsAnalyticsDashboard :is-admin="false" />
			</ShadTabsContent>
		</ShadTabs>
	</div>
</template>

<script setup lang="ts">
import {Copy} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
const activeProfileTab = ref('profile');

const userInitials = computed(() => {
	const username = store.auth.user?.username || 'U';
	return username.slice(0, 2).toUpperCase();
});
</script>
