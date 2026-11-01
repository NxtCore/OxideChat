import {defineStore} from 'pinia';
import {useMainStore} from './index';
import type {McpServer, UserPreferences, UpdatePreferencesRequest, StreamingAnimation} from '~/types/chat';

export const usePreferencesStore = defineStore('preferences', {
	state: () => ({
		mcpManagerOpen: false,
		userMcpServers: [] as McpServer[],
	}),

	actions: {
		async fetchUserMcpServers() {
			try {
				const {$customFetch} = useNuxtApp();
				const result = await $customFetch('/api/v1/mcp-servers');
				this.userMcpServers = (result as McpServer[]) ?? [];
			} catch (e) {
				console.error('Failed to fetch user MCP servers:', e);
			}
		},

		async fetchPreferences() {
			try {
				const {$customFetch} = useNuxtApp();
				const mainStore = useMainStore();
				const prefs = await $customFetch('/api/v1/users/@me/preferences');
				mainStore.preferences = prefs as UserPreferences;
			} catch (e) {
				console.error('Failed to fetch preferences:', e);
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
