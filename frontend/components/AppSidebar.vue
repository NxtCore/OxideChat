<template>
	<ShadSidebar collapsible="icon" class="border-r border-sidebar-border bg-sidebar/50">
		<ShadSidebarHeader class="border-b border-sidebar-border p-4">
			<div class="flex items-center gap-3">
				<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary">
					<MessageSquare class="h-5 w-5 text-primary-foreground" />
				</div>
				<div class="group-data-[collapsible=icon]:hidden">
					<h2 class="font-semibold text-foreground">OxideChat</h2>
					<p class="text-xs text-sidebar-foreground">{{ store.getTranslation('sidebar.ai_chat_app') }}</p>
				</div>
			</div>
		</ShadSidebarHeader>

		<ShadSidebarContent class="p-2">
			<ShadSidebarGroup>
				<ShadSidebarGroupLabel class="text-sidebar-foreground">{{ store.getTranslation('sidebar.chats') }}</ShadSidebarGroupLabel>
				<ShadSidebarGroupContent>
					<ShadSidebarMenu>
						<!-- Chat list placeholder -->
						<ShadSidebarMenuItem>
							<ShadSidebarMenuButton class="text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground">
								<Plus class="h-4 w-4" />
								<span class="group-data-[collapsible=icon]:hidden">{{ store.getTranslation('sidebar.new_chat') }}</span>
							</ShadSidebarMenuButton>
						</ShadSidebarMenuItem>
					</ShadSidebarMenu>
				</ShadSidebarGroupContent>
			</ShadSidebarGroup>
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
</template>

<script setup lang="ts">
import {MessageSquare, Plus, ChevronUp, LogOut, Settings} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
const router = useRouter();

const userInitials = computed(() => {
	const username = store.auth.user?.username || 'U';
	return username.slice(0, 2).toUpperCase();
});

function goToSettings() {
	router.push('/settings');
}

async function handleLogout() {
	await store.logout();
	router.push('/auth/login');
}
</script>
