<template>
	<ShadDialog :open="open" @update:open="$emit('update:open', $event)">
		<ShadDialogContent class="sm:max-w-[460px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('workspace.delete_title') }}</ShadDialogTitle>
				<ShadDialogDescription>
					{{ store.getTranslation('workspace.delete_description', {name: workspace?.name || ''}) }}
				</ShadDialogDescription>
			</ShadDialogHeader>

			<div class="space-y-2 py-2">
				<button
					v-for="opt in options"
					:key="opt.value"
					type="button"
					class="flex w-full items-start gap-3 rounded-md border p-3 text-left transition-colors"
					:class="action === opt.value ? 'border-primary bg-accent' : 'border-border hover:bg-accent/50'"
					@click="action = opt.value"
				>
					<component :is="opt.icon" class="mt-0.5 h-4 w-4 shrink-0" :class="opt.value === 'delete' ? 'text-destructive' : 'text-muted-foreground'" />
					<span class="text-sm" :class="opt.value === 'delete' ? 'text-destructive' : 'text-foreground'">{{ opt.label }}</span>
				</button>

				<div v-if="action === 'move'" class="pt-1">
					<ShadLabel class="mb-1 text-xs text-muted-foreground">{{ store.getTranslation('workspace.delete_move_target') }}</ShadLabel>
					<ShadSelect v-model="targetId">
						<ShadSelectTrigger class="h-9">
							<ShadSelectValue />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="ws in moveTargets" :key="ws.id" :value="ws.id">
								{{ ws.name }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
			</div>

			<ShadDialogFooter>
				<ShadButton variant="ghost" @click="$emit('update:open', false)">
					{{ store.getTranslation('workspace.cancel') }}
				</ShadButton>
				<ShadButton :variant="action === 'delete' ? 'destructive' : 'default'" :disabled="submitting || (action === 'move' && !targetId)" @click="confirm">
					{{ store.getTranslation('workspace.confirm') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {ref, computed, watch} from 'vue';
import {ArrowRightLeft, Archive, Trash2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useChatStore} from '@/stores/chatStore';
import type {Workspace, WorkspaceDeleteAction} from '~/types/chat';

const props = defineProps<{open: boolean; workspace: Workspace | null}>();
const emit = defineEmits<{(e: 'update:open', value: boolean): void}>();

const store = useMainStore();
const chatStore = useChatStore();

const action = ref<WorkspaceDeleteAction>('archive');
const targetId = ref<string | undefined>(undefined);
const submitting = ref(false);

const moveTargets = computed(() => chatStore.workspaces.filter(w => w.id !== props.workspace?.id));

const options = computed(() => [
	{value: 'move' as const, label: store.getTranslation('workspace.delete_action_move'), icon: ArrowRightLeft},
	{value: 'archive' as const, label: store.getTranslation('workspace.delete_action_archive'), icon: Archive},
	{value: 'delete' as const, label: store.getTranslation('workspace.delete_action_delete'), icon: Trash2},
]);

watch(
	() => props.open,
	isOpen => {
		if (isOpen) {
			action.value = 'archive';
			const preferred = chatStore.defaultWorkspace && chatStore.defaultWorkspace.id !== props.workspace?.id ? chatStore.defaultWorkspace : moveTargets.value[0];
			targetId.value = preferred?.id;
		}
	},
);

async function confirm() {
	if (!props.workspace) return;
	submitting.value = true;
	const ok = await chatStore.deleteWorkspace(props.workspace.id, {
		action: action.value,
		target_workspace_id: action.value === 'move' ? targetId.value : undefined,
	});
	submitting.value = false;
	if (ok) emit('update:open', false);
}
</script>
