import {defineStore} from 'pinia';
import {useMainStore} from './index';
import type {McpServer, UserPreferences, UpdatePreferencesRequest, StreamingAnimation} from '~/types/chat';
import type {Tool} from '~/types/tools';

export const usePreferencesStore = defineStore('preferences', {
	state: () => ({
		mcpManagerOpen: false,
		userMcpServers: [] as McpServer[],
		userTools: [] as Tool[],
		toolsLoaded: false,
		toolsStale: false,
	}),

	actions: {
		async ensureTools() {
			if (this.toolsLoaded && !this.toolsStale) return;
			this.toolsStale = false;
			try {
				const [tools] = await Promise.all([userTools(), this.fetchUserMcpServers()]);
				this.userTools = Array.isArray(tools) ? (tools as Tool[]) : [];
				this.toolsLoaded = true;
			} catch (e) {
				console.error('Failed to load tools:', e);
			}
		},

		async fetchUserMcpServers() {
			try {
				const {$customFetch} = useNuxtApp();
				const result = await $customFetch('/api/v1/mcp-servers');
				this.userMcpServers = (result as McpServer[]) ?? [];
			} catch (e) {
				console.error('Failed to fetch user MCP servers:', e);
			}
		},

		async updatePreferences(data: UpdatePreferencesRequest): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const mainStore = useMainStore();
				const updated = await $customFetch('/api/v1/users/@me/preferences', {
					method: 'PATCH',
					body: data,
				});
				mainStore.preferences = updated as UserPreferences;
				return true;
			} catch (e) {
				console.error('Failed to update preferences:', e);
				return false;
			}
		},

		setStreamingAnimation(animation: StreamingAnimation) {
			this.updatePreferences({streaming_animation: animation});
		},
	},
});
