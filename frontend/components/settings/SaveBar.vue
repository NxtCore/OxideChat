<template>
	<Transition
		enter-active-class="transition duration-200 ease-out"
		enter-from-class="translate-y-3 opacity-0"
		leave-active-class="transition duration-150 ease-in"
		leave-to-class="translate-y-3 opacity-0"
	>
		<div
			v-if="form.isDirty"
			data-testid="settings-save-bar"
			class="z-20 flex items-center justify-between gap-3 rounded-lg border border-border bg-popover px-4 py-2.5 text-popover-foreground shadow-lg max-lg:fixed max-lg:inset-x-3 max-lg:bottom-3 lg:sticky lg:bottom-3 lg:mt-6 lg:mb-3"
		>
			<span class="min-w-0 text-sm font-medium">{{ store.getTranslation('settings.save_bar.message') }}</span>
			<div class="flex shrink-0 items-center gap-2">
				<ShadButton variant="ghost" size="sm" :disabled="form.saving" data-testid="settings-save-bar-reset" @click="form.reset()">
					{{ store.getTranslation('settings.save_bar.reset') }}
				</ShadButton>
				<ShadButton size="sm" :disabled="form.saving" data-testid="settings-save-bar-save" @click="form.save()">
					<Loader2 v-if="form.saving" class="mr-2 h-4 w-4 animate-spin" />
					{{ store.getTranslation('common.save') }}
				</ShadButton>
			</div>
		</div>
	</Transition>

	<ShadDialog :open="form.leavePromptOpen" @update:open="open => !open && form.answerLeave(false)">
		<ShadDialogContent class="sm:max-w-md">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('settings.save_bar.leave_title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('settings.save_bar.leave_description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<ShadDialogFooter>
				<ShadButton variant="outline" data-testid="settings-leave-keep" @click="form.answerLeave(false)">
					{{ store.getTranslation('settings.save_bar.keep_editing') }}
				</ShadButton>
				<ShadButton variant="destructive" data-testid="settings-leave-discard" @click="form.answerLeave(true)">
					{{ store.getTranslation('settings.save_bar.discard') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import type {SettingsForm} from '~/composables/useSettingsForm';

const props = defineProps<{form: SettingsForm}>();

const store = useMainStore();

function saveOnShortcut(event: KeyboardEvent) {
	if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 's' || !props.form.isDirty) return;
	event.preventDefault();
	if (!document.querySelector('[role="dialog"]')) props.form.save();
}

onMounted(() => window.addEventListener('keydown', saveOnShortcut));
onBeforeUnmount(() => window.removeEventListener('keydown', saveOnShortcut));
</script>
