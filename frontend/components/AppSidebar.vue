<template>
	<ShadSidebar collapsible="icon" class="border-r border-zinc-700 bg-zinc-800/50">
		<ShadSidebarHeader class="border-b border-zinc-700 p-4">
			<div class="flex items-center gap-3">
				<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-violet-400 to-fuchsia-500">
					<MessageSquare class="h-5 w-5 text-white" />
				</div>
				<div class="group-data-[collapsible=icon]:hidden">
					<h2 class="font-semibold text-white">OxideChat</h2>
					<p class="text-xs text-zinc-400">AI Chat Application</p>
				</div>
			</div>
		</ShadSidebarHeader>

		<ShadSidebarContent class="p-2">
			<ShadSidebarGroup>
				<ShadSidebarGroupLabel class="text-zinc-400">Chats</ShadSidebarGroupLabel>
				<ShadSidebarGroupContent>
					<ShadSidebarMenu>
						<!-- Chat list placeholder -->
						<ShadSidebarMenuItem>
							<ShadSidebarMenuButton class="text-zinc-300 hover:bg-zinc-700/50 hover:text-white">
								<Plus class="h-4 w-4" />
								<span class="group-data-[collapsible=icon]:hidden">New Chat</span>
							</ShadSidebarMenuButton>
						</ShadSidebarMenuItem>
					</ShadSidebarMenu>
				</ShadSidebarGroupContent>
			</ShadSidebarGroup>
		</ShadSidebarContent>

		<ShadSidebarFooter class="border-t border-zinc-700 p-4">
			<ShadDropdownMenu>
				<ShadDropdownMenuTrigger as-child>
					<ShadSidebarMenuButton size="lg" class="w-full justify-start text-zinc-300 hover:bg-zinc-700/50 hover:text-white">
						<ShadAvatar class="h-8 w-8">
							<ShadAvatarFallback class="bg-gradient-to-br from-violet-400 to-fuchsia-500 text-white">
								{{ userInitials }}
							</ShadAvatarFallback>
						</ShadAvatar>
						<div class="flex flex-col items-start group-data-[collapsible=icon]:hidden">
							<span class="font-medium">{{ store.auth.user?.username || 'User' }}</span>
							<span class="text-xs text-zinc-400">{{ store.auth.user?.email }}</span>
						</div>
						<ChevronUp class="ml-auto h-4 w-4 group-data-[collapsible=icon]:hidden" />
					</ShadSidebarMenuButton>
				</ShadDropdownMenuTrigger>
				<ShadDropdownMenuContent side="top" class="w-56 border-zinc-700 bg-zinc-800" :side-offset="4">
					<ShadDropdownMenuLabel class="text-zinc-300">
						{{ store.auth.user?.email }}
					</ShadDropdownMenuLabel>
					<ShadDropdownMenuSeparator class="bg-zinc-700" />
					<ShadDropdownMenuItem class="text-zinc-300 focus:bg-zinc-700 focus:text-white" @click="handleLogout">
						<LogOut class="mr-2 h-4 w-4" />
						<span>Log out</span>
					</ShadDropdownMenuItem>
				</ShadDropdownMenuContent>
			</ShadDropdownMenu>
		</ShadSidebarFooter>
	</ShadSidebar>
</template>

<script setup lang="ts">
import {MessageSquare, Plus, ChevronUp, LogOut} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
const router = useRouter();

const userInitials = computed(() => {
	const username = store.auth.user?.username || 'U';
	return username.slice(0, 2).toUpperCase();
});

async function handleLogout() {
	await store.logout();
	router.push('/auth/login');
}
</script>
