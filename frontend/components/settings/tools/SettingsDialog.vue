<template>
	<Dialog v-model:open="open">
		<DialogContent class="sm:max-w-[440px]">
			<DialogHeader>
				<DialogTitle class="flex items-center gap-3">
					<Sparkles v-if="isImagegenSettings" class="h-5 w-5 text-primary" />
					<Key v-else class="h-5 w-5 text-primary" />
					<span>{{ isImagegenSettings ? imageModelTitle : store.getTranslation('settings.tools.settings') }}</span>
				</DialogTitle>
				<DialogDescription>
					{{
						isImagegenSettings
							? imageModelDescription
							: store.getTranslation('settings.tools.settings_description', {name: tool?.display_name || tool?.name})
					}}
				</DialogDescription>
			</DialogHeader>

			<div class="space-y-4 py-4">
				<div v-for="(field, key) in settingsFields" :key="key" class="space-y-2">
					<Label :for="`setting-${key}`">{{ field.title || key }}</Label>
					<template v-if="field.format === 'model-picker'">
						<DefaultModelPicker
							v-model="userSettings[key]"
							:endpoint="adminImageModelsPath"
							:provider-endpoint="adminImageModelProvidersPath"
							:selected-model-endpoint="adminModelsPath"
							value-mode="uuid"
							:placeholder="imageModelPlaceholder"
						/>
					</template>
					<template v-else-if="field.enum && field.enum.length > 0">
						<ShadSelect v-model="userSettings[key]">
							<ShadSelectTrigger :id="`setting-${key}`">
								<ShadSelectValue :placeholder="field.description || store.getTranslation('settings.tools.select_placeholder')" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="option in field.enum" :key="option" :value="option">
									{{ option }}
								</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</template>
					<template v-else>
						<Input :id="`setting-${key}`" v-model="userSettings[key]" :type="field.secret ? 'password' : 'text'" :placeholder="field.description || ''" />
					</template>
					<p v-if="field.description && field.enum" class="text-xs text-muted-foreground">{{ field.description }}</p>
				</div>
			</div>

			<DialogFooter>
				<ShadButton variant="outline" @click="open = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton @click="saveSettings" :disabled="saving">
					<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
					{{ store.getTranslation('common.save') }}
				</ShadButton>
			</DialogFooter>
		</DialogContent>
	</Dialog>
</template>

<script setup lang="ts">
import {Sparkles, Key, Loader2} from 'lucide-vue-next';
import DefaultModelPicker from '@/components/settings/DefaultModelPicker.vue';
import {useMainStore} from '@/stores';
import type {Tool} from '~/types/tools';
import {Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Input} from '@/components/ui/input';
import {Label} from '@/components/ui/label';

const props = defineProps<{
	tool: Tool | null;
}>();

const emit = defineEmits<{
	saved: [];
}>();

const open = defineModel<boolean>('open', {default: false});

const store = useMainStore();

const saving = ref(false);
const userSettings = ref<Record<string, string>>({});

const settingsFields = computed(() => props.tool?.settings_schema?.properties ?? {});

const isImagegenSettings = computed(() => props.tool?.source_config?.builtin_id === 'imagegen');

const imageModelTitle = computed(() => translateOr('settings.tools.image_model_title', store.getTranslation('settings.tools.settings')));
const imageModelDescription = computed(() => translateOr('settings.tools.image_model_description', ''));
const imageModelPlaceholder = computed(() => translateOr('settings.tools.image_model_placeholder', store.getTranslation('settings.tools.select_placeholder')));

watch(open, async isOpen => {
	if (!isOpen || !props.tool) return;
	try {
		userSettings.value = ((await toolSettings(props.tool.id)) as Record<string, string>) || {};
	} catch {
		userSettings.value = {};
	}
});

async function saveSettings() {
	if (!props.tool) return;
	saving.value = true;
	try {
		await saveToolSettings(props.tool.id, {settings: userSettings.value});
		store.toast('Settings saved', {type: 'success'});
		open.value = false;
		emit('saved');
	} catch (e: any) {
		store.toast('Failed to save settings', {type: 'error', description: e.message});
	} finally {
		saving.value = false;
	}
}
</script>
