<template>
	<div class="max-w-4xl lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between">
			<div class="mb-6">
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.providers.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.providers.description') }}</p>
			</div>
			<ShadButton variant="default" size="sm" class="gap-2" @click="addCustomProvider">
				<Plus class="h-4 w-4" />
				<span>{{ store.getTranslation('settings.providers.add_custom') }}</span>
			</ShadButton>
		</div>

		<div class="space-y-3">
			<div
				v-for="item in displayProviders"
				:key="item.id || item.kind + item.name"
				class="rounded-lg border border-border bg-card p-4 transition-all hover:border-border/80"
			>
				<div class="flex items-center justify-between gap-4">
					<div class="flex items-center gap-4 min-w-0">
						<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg">
							<div
								v-if="item.template?.icon && item.template.icon.type === 'svg'"
								v-html="item.template.icon.icon"
								class="h-5 w-5 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
								:style="{color: item.template.brandColor}"
							></div>
							<div v-else-if="item.template?.icon && item.template.icon.type === 'png'" class="h-5 w-5">
								<img :src="item.template.icon.icon" :alt="item.name" />
							</div>
							<BrainCircuit v-else class="h-5 w-5" :style="{color: '#6366f1'}" />
						</div>
						<div class="min-w-0">
							<div class="flex items-center gap-2">
								<h3 class="font-medium text-foreground">{{ item.name }}</h3>
								<span v-if="item.isConfigured" class="inline-flex items-center rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
									{{ store.getTranslation('settings.providers.configured') }}
								</span>
							</div>
							<p class="text-sm text-muted-foreground truncate">{{ item.description || item.base_url }}</p>
						</div>
					</div>
					<div class="shrink-0 flex items-center gap-2">
						<Button variant="outline" size="sm" class="gap-2" @click="openConfigDialog(item)">
							<Settings2 class="h-4 w-4" />
							<span v-if="!item.isConfigured">{{ store.getTranslation('settings.providers.configure') }}</span>
						</Button>
						<Button v-if="item.isConfigured" variant="outline" size="sm" class="gap-2" @click="syncProvider(item)">
							<RotateCw class="h-4 w-4" />
						</Button>
						<Switch :modelValue="item.is_enabled || false" :disabled="!item.isConfigured" @update:modelValue="(val: boolean) => toggleProvider(item, val)" />
					</div>
				</div>
			</div>
		</div>

		<Dialog v-model:open="dialogOpen">
			<DialogContent class="sm:max-w-[500px]">
				<DialogHeader>
					<DialogTitle class="flex items-center gap-3">
						<div
							v-if="selectedProvider"
							class="flex h-8 w-8 items-center justify-center rounded-lg"
							:style="{backgroundColor: selectedProvider.brandColor + '15'}"
						>
							<div
								v-if="selectedProvider.icon?.type === 'svg'"
								v-html="selectedProvider.icon.icon"
								class="h-5 w-5"
								:style="{color: selectedProvider.brandColor}"
							></div>
							<div v-else-if="selectedProvider.icon?.type === 'png'" class="h-5 w-5">
								<img :src="selectedProvider.icon?.icon" :alt="selectedProvider.name" />
							</div>
						</div>
						<span>{{ store.getTranslation('settings.providers.configure') }} {{ selectedProvider?.name }}</span>
					</DialogTitle>
					<DialogDescription>
						{{ store.getTranslation('settings.providers.configure_description') }}
					</DialogDescription>
				</DialogHeader>

				<div class="space-y-4 py-4">
					<div v-if="!selectedProvider?.isPreConfigured" class="space-y-2">
						<Label for="provider-name">{{ store.getTranslation('settings.providers.name') }}</Label>
						<Input id="provider-name" v-model="configForm.name" type="text" :placeholder="selectedProvider?.name" />
					</div>

					<div class="space-y-2">
						<Label for="api-key">{{ store.getTranslation('settings.providers.api_key') }}</Label>
						<Input
							id="api-key"
							v-model="configForm.apiKey"
							type="password"
							:placeholder="configForm.existingProvider ? '••••••••' : store.getTranslation('settings.providers.api_key_placeholder')"
						/>
						<p class="text-xs text-muted-foreground">
							{{ store.getTranslation('settings.providers.api_key_hint') }}
						</p>
					</div>

					<div v-if="!selectedProvider?.isPreConfigured" class="space-y-2">
						<Label for="base-url">{{ store.getTranslation('settings.providers.base_url') }}</Label>
						<Input id="base-url" v-model="configForm.baseUrl" type="text" :placeholder="selectedProvider?.defaultBaseUrl || 'https://api.example.com'" />
					</div>
				</div>

				<DialogFooter class="gap-2 sm:gap-0">
					<Button v-if="configForm.existingProvider" variant="destructive" @click="deleteConfig" :disabled="saving" class="mr-auto">
						<Trash2 class="h-4 w-4 mr-2" />
						{{ store.getTranslation('common.delete') }}
					</Button>
					<div class="flex flex-row gap-2">
						<Button variant="outline" @click="dialogOpen = false">
							{{ store.getTranslation('common.cancel') }}
						</Button>
						<Button @click="saveConfig" :disabled="saving">
							<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
							{{ store.getTranslation('common.save') }}
						</Button>
					</div>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	</div>
</template>

<script setup lang="ts">
import {ref, reactive, onMounted, computed} from 'vue';
import {Sparkles, Cpu, Zap, Server, Plus, Settings2, Loader2, BrainCircuit, Globe, AudioWaveform, Trash2, RotateCw} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {Button} from '@/components/ui/button';
import {Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Input} from '@/components/ui/input';
import {Label} from '@/components/ui/label';
import {Switch} from '@/components/ui/switch';
const {$customFetch} = useNuxtApp();

interface ProviderConfig {
	kind: string;
	name: string;
	description: string;
	icon: any;
	brandColor: string;
	defaultBaseUrl?: string;
	isPreConfigured: boolean;
	keyFormat?: string;
}

interface ConfiguredProvider {
	id: string;
	kind: string;
	name: string;
	base_url: string;
	has_api_key: boolean;
	is_enabled: boolean;
}

const store = useMainStore();
const iconsStore = useIconsStore();
const dialogOpen = ref(false);
const selectedProvider = ref<ProviderConfig | null>(null);
const saving = ref(false);
const configuredProviders = ref<ConfiguredProvider[]>([]);

const displayProviders = computed(() => {
	const result: any[] = [];

	// 1. Add all configured providers
	for (const conf of configuredProviders.value) {
		const provider_template = providers.find(p => {
			if (p.kind !== conf.kind) return false;
			if (p.kind === 'OPENAI_COMPAT' && p.isPreConfigured) {
				return conf.base_url === p.defaultBaseUrl;
			}
			return true;
		});

		result.push({
			...conf,
			isConfigured: true,
			template: provider_template || {
				name: conf.name,
				brandColor: '#fff',
				isPreConfigured: false,
				icon: iconsStore.getProviderIcon(conf.name),
			},
		});
	}

	// 2. Add templates that aren't configured yet
	for (const template of providers) {
		const isAlreadyAdded = result.some(p => {
			if (p.kind !== template.kind) return false;
			if (template.kind === 'openai_compat') {
				return p.base_url === template.defaultBaseUrl;
			}
			return true;
		});

		if (!isAlreadyAdded) {
			result.push({
				kind: template.kind,
				name: template.name,
				description: template.description,
				template,
				isConfigured: false,
				is_enabled: false,
			});
		}
	}

	return result;
});

const configForm = reactive({
	name: '',
	apiKey: '',
	baseUrl: '',
	isEnabled: true,
	existingProvider: null as ConfiguredProvider | null,
});

const providers: ProviderConfig[] = [
	{
		kind: 'OPENROUTER',
		name: 'OpenRouter',
		description: store.getTranslation('settings.providers.openrouter_description'),
		icon: iconsStore.getProviderIcon('openrouter'),
		brandColor: '#fff',
		defaultBaseUrl: 'https://openrouter.ai/api',
		isPreConfigured: true,
		keyFormat: 'sk-or-v1-',
	},
	{
		kind: 'OPENAI',
		name: 'OpenAI',
		description: store.getTranslation('settings.providers.openai_description'),
		icon: iconsStore.getProviderIcon('openai'),
		brandColor: '#fff',
		defaultBaseUrl: 'https://api.openai.com',
		isPreConfigured: true,
		keyFormat: 'sk-proj-',
	},
	{
		kind: 'ANTHROPIC',
		name: 'Anthropic',
		description: store.getTranslation('settings.providers.anthropic_description'),
		icon: iconsStore.getProviderIcon('anthropic'),
		brandColor: '#fff',
		defaultBaseUrl: 'https://api.anthropic.com',
		isPreConfigured: true,
		keyFormat: 'sk-ant-',
	},
	{
		kind: 'GOOGLE',
		name: 'Google',
		description: store.getTranslation('settings.providers.google_description'),
		icon: iconsStore.getProviderIcon('google'),
		brandColor: '#fff',
		defaultBaseUrl: 'https://generativelanguage.googleapis.com',
		isPreConfigured: true,
		keyFormat: 'AIza',
	},
];

async function loadProviders() {
	try {
		const result = await $customFetch('/api/v1/admin/providers');
		if (Array.isArray(result)) {
			configuredProviders.value = result;
		}
	} catch (e: any) {
		console.error('Failed to load providers:', e);
	}
}

async function toggleProvider(item: any, enabled: boolean) {
	if (item.isConfigured) {
		const toast = store.toast(store.getTranslation('settings.providers.toggling_provider'), {
			description: store.getTranslation('settings.providers.toggling_provider_description'),
			type: 'loading',
			duration: Infinity,
		});
		await updateProvider(item.id, {is_enabled: enabled});
		store.dismissToast(toast);
		store.toast(store.getTranslation('settings.providers.toggling_provider_success'), {type: 'success'});
	}
}

function openConfigDialog(item: any) {
	selectedProvider.value = item.template || {
		kind: item.kind,
		name: item.name,
		brandColor: '#6366f1',
		isPreConfigured: false,
	};

	if (item.isConfigured) {
		configForm.name = item.name;
		configForm.apiKey = '';
		configForm.baseUrl = item.base_url;
		configForm.isEnabled = item.is_enabled;
		configForm.existingProvider = item;
	} else {
		configForm.name = item.name;
		configForm.apiKey = '';
		configForm.baseUrl = item.template?.defaultBaseUrl || '';
		configForm.isEnabled = true;
		configForm.existingProvider = null;
	}

	dialogOpen.value = true;
}

function addCustomProvider() {
	selectedProvider.value = {
		kind: 'OPENAI_COMPAT',
		name: '',
		description: '',
		icon: null,
		brandColor: '#6366f1',
		isPreConfigured: false,
	};
	configForm.name = '';
	configForm.apiKey = '';
	configForm.baseUrl = '';
	configForm.isEnabled = true;
	configForm.existingProvider = null;
	dialogOpen.value = true;
}

async function updateProvider(id: string, data: Object) {
	const provider = configuredProviders.value.find(p => p.id === id);
	if (!provider) return;

	try {
		await $customFetch(`/api/v1/admin/providers/${id}`, {
			method: 'PUT',
			body: data,
		});
	} catch (e: any) {
		store.toast(store.getTranslation('settings.providers.save_error'), {
			type: 'error',
			description: e.message || e.toString(),
		});
		console.error('Failed to update provider:', e);
	}
	await loadProviders();
}

async function syncProvider(provider: any) {
	if (!provider || !provider.id) {
		console.error('Invalid provider:', provider);
		return;
	}
	const toast = store.toast(store.getTranslation('settings.providers.syncing_provider'), {
		description: store.getTranslation('settings.providers.syncing_provider_description'),
		type: 'loading',
		duration: Infinity,
	});
	await $customFetch(`/api/v1/admin/providers/${provider.id}/sync`, {
		method: 'POST',
	});
	console.log(toast);
	store.dismissToast(toast);
	store.toast(store.getTranslation('settings.providers.syncing_provider_success'), {type: 'success'});
}
async function saveConfig() {
	if (!selectedProvider.value) return;

	saving.value = true;
	try {
		const body: any = {
			kind: selectedProvider.value.kind,
			name: configForm.name || selectedProvider.value.name,
			base_url: configForm.baseUrl || selectedProvider.value.defaultBaseUrl || '',
			is_enabled: configForm.isEnabled,
		};

		if (configForm.apiKey) {
			body.api_key = configForm.apiKey;
		}

		if (configForm.existingProvider) {
			await $customFetch(`/api/v1/admin/providers/${configForm.existingProvider.id}`, {
				method: 'PUT',
				body,
			});
		} else {
			await $customFetch('/api/v1/admin/providers', {
				method: 'POST',
				body,
			});
		}

		store.toast(store.getTranslation('settings.providers.save_success'), {type: 'success'});
		dialogOpen.value = false;
		await loadProviders();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.providers.save_error'), {
			type: 'error',
			description: e.message || e.toString(),
		});
	} finally {
		saving.value = false;
	}
}

async function deleteConfig() {
	if (!configForm.existingProvider) return;

	saving.value = true;
	try {
		await $customFetch(`/api/v1/admin/providers/${configForm.existingProvider.id}`, {
			method: 'DELETE',
		});
		store.toast(store.getTranslation('settings.providers.delete_success'), {type: 'success'});
		dialogOpen.value = false;
		await loadProviders();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.providers.delete_error'), {
			type: 'error',
			description: e.message || e.toString(),
		});
	} finally {
		saving.value = false;
	}
}

onMounted(() => {
	loadProviders();
});
</script>
