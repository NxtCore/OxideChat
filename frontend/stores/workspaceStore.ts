import {defineStore} from 'pinia';
import {useMainStore} from './index';
import {useThemeStore} from './theme';
import {useChatStore} from './chatStore';
import type {Workspace, CreateWorkspaceRequest, UpdateWorkspaceRequest, DeleteWorkspaceOptions} from '~/types/chat';

const ACTIVE_WORKSPACE_KEY = 'oxide-active-workspace';

function loadActiveWorkspaceId(): string | null {
	if (typeof window === 'undefined') return null;
	try {
		return localStorage.getItem(ACTIVE_WORKSPACE_KEY);
	} catch {
		return null;
	}
}

function persistActiveWorkspaceId(id: string | null) {
	if (typeof window === 'undefined') return;
	try {
		if (id) localStorage.setItem(ACTIVE_WORKSPACE_KEY, id);
		else localStorage.removeItem(ACTIVE_WORKSPACE_KEY);
	} catch {}
}

export const useWorkspaceStore = defineStore('workspace', {
	state: () => ({
		workspaces: [] as Workspace[],
		activeWorkspaceId: null as string | null,
		workspacesLoading: false,
	}),

	getters: {
		defaultWorkspace(): Workspace | undefined {
			return this.workspaces.find(w => w.is_default);
		},

		activeWorkspace(): Workspace | undefined {
			if (!this.activeWorkspaceId) return this.defaultWorkspace;
			return this.workspaces.find(w => w.id === this.activeWorkspaceId);
		},
	},

	actions: {
		async fetchWorkspaces() {
			this.workspacesLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const workspaces = await $customFetch('/api/v1/workspaces');
				this.workspaces = workspaces as Workspace[];
			} catch (e) {
				console.error('Failed to fetch workspaces:', e);
			} finally {
				this.workspacesLoading = false;
			}
		},

		async createWorkspace(data: CreateWorkspaceRequest): Promise<Workspace | null> {
			try {
				const {$customFetch} = useNuxtApp();
				const workspace = await $customFetch('/api/v1/workspaces', {
					method: 'POST',
					body: data,
				});
				this.workspaces.push(workspace as Workspace);
				return workspace as Workspace;
			} catch (e) {
				console.error('Failed to create workspace:', e);
				this.notifyWorkspaceError(e, 'workspace.create_failed');
				return null;
			}
		},

		notifyWorkspaceError(e: any, fallbackKey: string) {
			const mainStore = useMainStore();
			const message = e?.data?.errors?.[0]?.message || mainStore.getTranslation(fallbackKey);
			mainStore.toast(message, {type: 'error'});
		},

		async updateWorkspace(id: string, data: UpdateWorkspaceRequest): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const updated = await $customFetch(`/api/v1/workspaces/${id}`, {
					method: 'PATCH',
					body: data,
				});
				const index = this.workspaces.findIndex(w => w.id === id);
				if (index !== -1) this.workspaces[index] = updated as Workspace;
				if (this.activeWorkspaceId === id) this.applyWorkspaceAccent();
				return true;
			} catch (e) {
				console.error('Failed to update workspace:', e);
				this.notifyWorkspaceError(e, 'workspace.update_failed');
				return false;
			}
		},

		async deleteWorkspace(id: string, opts: DeleteWorkspaceOptions): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const query = new URLSearchParams({action: opts.action});
				if (opts.action === 'move' && opts.target_workspace_id) query.set('target_workspace_id', opts.target_workspace_id);
				await $customFetch(`/api/v1/workspaces/${id}?${query.toString()}`, {method: 'DELETE'});
				this.workspaces = this.workspaces.filter(w => w.id !== id);
				if (this.activeWorkspaceId === id) {
					this.activeWorkspaceId = null;
					persistActiveWorkspaceId(null);
					this.applyWorkspaceAccent();
				}
				await this.fetchWorkspaces();
				await useChatStore().fetchChats({workspace_id: this.activeWorkspaceId || undefined});
				return true;
			} catch (e) {
				console.error('Failed to delete workspace:', e);
				this.notifyWorkspaceError(e, 'workspace.delete_failed');
				return false;
			}
		},

		setActiveWorkspace(id: string | null) {
			this.activeWorkspaceId = id;
			persistActiveWorkspaceId(id);
			this.applyWorkspaceAccent();
			useChatStore().fetchChats({workspace_id: id || undefined});
		},

		applyWorkspaceAccent() {
			const active = this.activeWorkspaceId ? this.workspaces.find(w => w.id === this.activeWorkspaceId) : undefined;
			useThemeStore().setWorkspaceAccent(active?.color ?? null);
		},

		restoreActive() {
			this.activeWorkspaceId = loadActiveWorkspaceId();
		},

		clearActiveIfMissing() {
			if (this.activeWorkspaceId && !this.workspaces.some(w => w.id === this.activeWorkspaceId)) {
				this.activeWorkspaceId = null;
				persistActiveWorkspaceId(null);
			}
		},
	},
});
