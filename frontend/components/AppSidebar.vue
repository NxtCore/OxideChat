<template>
	<ShadSidebar collapsible="icon" class="border-r border-sidebar-border bg-sidebar/50">
		<ShadSidebarHeader class="border-b border-sidebar-border p-4">
			<div class="flex items-center gap-3">
				<img src="/light_transparent.svg" alt="OxideChat Logo" class="h-6 w-6" />
				<div class="group-data-[collapsible=icon]:hidden">
					<h2 class="font-semibold text-foreground">OxideChat</h2>
				</div>
			</div>
		</ShadSidebarHeader>

		<ShadSidebarContent class="p-2">
			<ChatList />
		</ShadSidebarContent>

		<ShadSidebarFooter class="border-t border-sidebar-border p-4">
			<ShadDropdownMenu>
				<ShadDropdownMenuTrigger as-child>
					<ShadSidebarMenuButton size="lg" class="w-full justify-start text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground">
						<ShadAvatar class="h-8 w-8">
							<ShadAvatarFallback class="bg-primary text-primary-foreground">
								{{ userInitials }}
							</ShadAvatarFallback>
						</ShadAvatar>
						<div class="flex flex-col items-start group-data-[collapsible=icon]:hidden">
							<span class="font-medium">{{ store.auth.user?.username || store.getTranslation('common.user') }}</span>
							<span class="text-xs text-sidebar-foreground">{{ store.auth.user?.email }}</span>
						</div>
						<ChevronUp class="ml-auto h-4 w-4 group-data-[collapsible=icon]:hidden" />
					</ShadSidebarMenuButton>
				</ShadDropdownMenuTrigger>
				<ShadDropdownMenuContent side="top" class="w-56 border-border bg-popover" :side-offset="4">
					<ShadDropdownMenuLabel class="text-popover-foreground">
						{{ store.auth.user?.email }}
					</ShadDropdownMenuLabel>
					<ShadDropdownMenuSeparator class="bg-border" />
					<ShadDropdownMenuSub>
						<ShadDropdownMenuSubTrigger class="text-popover-foreground">
							<Layers class="mr-2 h-4 w-4" />
							<span>{{ store.getTranslation('sidebar.workspace') }}</span>
							<span class="ml-auto text-xs text-muted-foreground">
								{{ chatStore.activeWorkspace?.name || store.getTranslation('sidebar.all') }}
							</span>
						</ShadDropdownMenuSubTrigger>
						<ShadDropdownMenuSubContent class="border-border bg-popover">
							<ShadDropdownMenuItem class="text-popover-foreground focus:bg-accent" @click="chatStore.setActiveWorkspace(null)">
								<span :class="!chatStore.activeWorkspaceId ? 'font-medium' : ''">{{ store.getTranslation('sidebar.all_chats') }}</span>
							</ShadDropdownMenuItem>
							<ShadDropdownMenuSeparator class="bg-border" />
							<ShadDropdownMenuItem
								v-for="workspace in chatStore.workspaces"
								:key="workspace.id"
								class="text-popover-foreground focus:bg-accent"
								@click="chatStore.setActiveWorkspace(workspace.id)"
							>
								<span class="mr-2 h-2.5 w-2.5 shrink-0 rounded-full border border-border" :style="{backgroundColor: workspace.color || 'var(--muted)'}" />
								<span :class="workspace.id === chatStore.activeWorkspaceId ? 'font-medium' : ''">
									{{ workspace.name }}
								</span>
								<span class="ml-auto text-xs text-muted-foreground">{{ workspace.chat_count }}</span>
							</ShadDropdownMenuItem>
							<ShadDropdownMenuSeparator class="bg-border" />
							<ShadDropdownMenuItem class="text-popover-foreground focus:bg-accent" @click="showWorkspaceManager = true">
								<Settings2 class="mr-2 h-4 w-4" />
								<span>{{ store.getTranslation('sidebar.manage_workspaces') }}</span>
							</ShadDropdownMenuItem>
						</ShadDropdownMenuSubContent>
					</ShadDropdownMenuSub>

					<template v-if="budgetStore.myStatus?.budgets.length">
						<ShadDropdownMenuSeparator class="bg-border" />
						<div class="px-2 py-2">
							<div class="mb-1.5 flex items-center justify-between text-xs text-muted-foreground">
								<span>{{ store.getTranslation('settings.tabs.budgets') }}</span>
								<span :class="budgetStore.highestUsagePercent >= 90 ? 'text-destructive' : ''">
									{{ store.getTranslation('sidebar.budget.remaining', {amount: formatMoney(budgetStore.lowestRemaining ?? 0)}) }}
								</span>
							</div>
							<div class="h-1.5 rounded-full bg-muted">
								<div
									class="h-1.5 rounded-full transition-all"
									:class="budgetStore.highestUsagePercent >= 90 ? 'bg-destructive' : 'bg-primary'"
									:style="{width: `${budgetStore.highestUsagePercent}%`}"
								/>
							</div>
						</div>
					</template>

					<ShadDropdownMenuSeparator class="bg-border" />
					<ShadDropdownMenuItem class="text-popover-foreground focus:bg-accent focus:text-accent-foreground" @click="goToSettings">
						<Settings class="mr-2 h-4 w-4" />
						<span>{{ store.getTranslation('settings.title') }}</span>
					</ShadDropdownMenuItem>
					<ShadDropdownMenuItem class="text-popover-foreground focus:bg-accent focus:text-accent-foreground" @click="handleLogout">
						<LogOut class="mr-2 h-4 w-4" />
						<span>{{ store.getTranslation('sidebar.logout') }}</span>
					</ShadDropdownMenuItem>
				</ShadDropdownMenuContent>
			</ShadDropdownMenu>
		</ShadSidebarFooter>
	</ShadSidebar>

	<WorkspaceManagerDialog v-model:open="showWorkspaceManager" />
</template>

<script setup lang="ts">
import {ChevronUp, LogOut, Settings, Settings2, Layers} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useChatStore} from '@/stores/chatStore';
import {useBudgetStore} from '@/stores/budgetStore';

const store = useMainStore();
const chatStore = useChatStore();
const budgetStore = useBudgetStore();
const router = useRouter();
const route = useRoute();

const showWorkspaceManager = ref(false);

const userInitials = computed(() => {
	const username = store.auth.user?.username || 'U';
	return username.slice(0, 2).toUpperCase();
});

function goToSettings() {
	store.settingsReturnPath = route.path.startsWith('/chats/') ? route.fullPath : '/';
	router.push('/settings');
}

async function handleLogout() {
	await store.logout();
	router.push('/auth/login');
}

function formatMoney(value: number) {
	return `$${value.toFixed(2)}`;
}

onMounted(() => {
	if (store.auth.isAuthenticated) {
		budgetStore.fetchMyBudget().catch(() => {});
	}
});
</script>
