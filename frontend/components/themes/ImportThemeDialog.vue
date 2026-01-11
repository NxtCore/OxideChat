<script setup lang="ts">
import { ref } from 'vue';
import { useThemeStore } from '~/stores/theme';
import { useMainStore } from '~/stores';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog';
import { Button } from '~/components/ui/button';
import { Input } from '~/components/ui/input';
import { Label } from '~/components/ui/label';
import { ExternalLink, Loader2 } from 'lucide-vue-next';

const props = defineProps<{
	open: boolean;
}>();

const emit = defineEmits<{
	(e: 'update:open', value: boolean): void;
	(e: 'imported'): void;
}>();

const themeStore = useThemeStore();
const mainStore = useMainStore();

const themeUrl = ref('');
const isLoading = ref(false);
const error = ref('');

async function handleImport() {
	if (!themeUrl.value.trim()) return;

	isLoading.value = true;
	error.value = '';

	try {
		await themeStore.importTheme(themeUrl.value.trim());
		mainStore.toast(mainStore.getTranslation('settings.theme_imported'), { type: 'success' });
		emit('imported');
		emit('update:open', false);
		themeUrl.value = '';
	} catch (e: any) {
		error.value = e.message || 'Failed to import theme';
		mainStore.toast(error.value, { type: 'error' });
	} finally {
		isLoading.value = false;
	}
}
</script>

<template>
	<Dialog :open="open" @update:open="emit('update:open', $event)">
		<DialogContent class="sm:max-w-md">
			<DialogHeader>
				<DialogTitle>{{ mainStore.getTranslation('settings.import_theme') }}</DialogTitle>
				<DialogDescription>
					{{ mainStore.getTranslation('settings.import_theme_description') }}
					<a
						href="https://tweakcn.com"
						target="_blank"
						rel="noopener noreferrer"
						class="inline-flex items-center gap-1 text-primary hover:underline ml-1"
					>
						tweakcn.com
						<ExternalLink class="w-3 h-3" />
					</a>
				</DialogDescription>
			</DialogHeader>

			<div class="space-y-4 py-4">
				<div class="space-y-2">
					<Label for="theme-url">{{ mainStore.getTranslation('settings.theme_url') }}</Label>
					<Input
						id="theme-url"
						v-model="themeUrl"
						placeholder="https://tweakcn.com/themes/themeId"
						:disabled="isLoading"
						@keyup.enter="handleImport"
					/>
				</div>
				<p v-if="error" class="text-sm text-destructive">{{ error }}</p>
			</div>

			<DialogFooter>
				<Button variant="outline" @click="emit('update:open', false)" :disabled="isLoading">
					{{ mainStore.getTranslation('common.cancel') }}
				</Button>
				<Button @click="handleImport" :disabled="isLoading || !themeUrl.trim()">
					<Loader2 v-if="isLoading" class="w-4 h-4 mr-2 animate-spin" />
					{{ mainStore.getTranslation('settings.import') }}
				</Button>
			</DialogFooter>
		</DialogContent>
	</Dialog>
</template>
