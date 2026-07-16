<template>
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
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
				<div v-if="item.isConfigured && item.billing" class="mt-4 border-t border-border pt-3">
					<div class="flex flex-wrap items-center justify-between gap-2">
						<div class="flex flex-wrap items-center gap-2">
							<span class="inline-flex rounded-full bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground">
								{{ store.getTranslation(billingStatusKey(item.billing)) }}
							</span>
							<span v-if="item.billing.is_stale" class="inline-flex rounded-full bg-destructive/10 px-2 py-0.5 text-xs font-medium text-destructive">
								{{ store.getTranslation('settings.providers.billing.stale') }}
							</span>
						</div>
						<Button
							v-if="item.billing.is_enabled"
							variant="ghost"
							size="sm"
							:disabled="billingStore.loading[item.id]"
							@click="refreshBilling(item.id)"
						>
							<Loader2 v-if="billingStore.loading[item.id]" class="mr-2 h-3.5 w-3.5 animate-spin" />
							<RotateCw v-else class="mr-2 h-3.5 w-3.5" />
							{{ store.getTranslation('settings.providers.billing.refresh') }}
						</Button>
					</div>
					<div v-if="item.billing.upstream" class="mt-2 space-y-1.5 text-sm">
						<p class="font-medium text-foreground">{{ upstreamSummary(item.billing) }}</p>
						<p v-if="upstreamDetail(item.billing)" class="text-muted-foreground">{{ upstreamDetail(item.billing) }}</p>
						<div v-if="item.billing.upstream.limit_amount != null" class="h-2 overflow-hidden rounded-full bg-muted">
							<div class="h-full rounded-full bg-primary transition-all" :style="{width: billingProgress(item.billing) + '%'}"></div>
						</div>
					</div>
					<div class="mt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
						<span>
							{{ store.getTranslation('settings.providers.billing.local_tracked') }}:
							{{ store.getTranslation('settings.providers.billing.local_month', {amount: formatMoney(item.billing.local.spent_amount, item.billing.local.currency)}) }}
						</span>
						<span>{{ updatedLabel(item.billing.last_synced_at) }}</span>
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

				<Tabs v-model="activeProviderTab" default-value="settings" class="w-full">
					<TabsList v-if="configForm.existingProvider" class="grid w-full mt-2" :class="showCatalogTab ? 'grid-cols-3' : 'grid-cols-2'">
						<TabsTrigger value="settings">{{ store.getTranslation('settings.providers.tab_settings') }}</TabsTrigger>
						<TabsTrigger value="billing">{{ store.getTranslation('settings.providers.billing.tab') }}</TabsTrigger>
						<TabsTrigger v-if="showCatalogTab" value="catalog">{{ store.getTranslation('settings.providers.tab_catalog') }}</TabsTrigger>
					</TabsList>

					<TabsContent value="settings">
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
					</TabsContent>

					<TabsContent v-if="configForm.existingProvider" value="billing">
						<div class="space-y-4 py-4">
							<div v-if="supportsUpstreamBilling" class="space-y-4">
								<div class="flex items-center justify-between gap-4">
									<Label for="billing-enabled">{{ store.getTranslation('settings.providers.billing.enable') }}</Label>
									<Switch id="billing-enabled" v-model="billingForm.isEnabled" />
								</div>
								<template v-if="selectedProviderKind === 'OPENAI'">
									<div class="space-y-2">
										<Label for="billing-key">{{ store.getTranslation('settings.providers.billing.admin_key') }}</Label>
										<Input id="billing-key" v-model="billingForm.credential" type="password" :placeholder="billingForm.hasCredential ? '••••••••' : ''" />
										<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.providers.billing.admin_key_hint') }}</p>
									</div>
									<div class="space-y-2">
										<Label for="billing-project">{{ store.getTranslation('settings.providers.billing.project_id') }}</Label>
										<Input id="billing-project" v-model="billingForm.scopeId" />
									</div>
									<div class="space-y-2">
										<Label for="billing-project-name">{{ store.getTranslation('settings.providers.billing.project_name') }}</Label>
										<Input id="billing-project-name" v-model="billingForm.scopeName" />
									</div>
								</template>
								<div v-else class="space-y-2">
									<Label for="billing-key">{{ store.getTranslation('settings.providers.billing.management_key') }}</Label>
									<Input id="billing-key" v-model="billingForm.credential" type="password" :placeholder="billingForm.hasCredential ? '••••••••' : ''" />
									<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.providers.billing.management_key_hint') }}</p>
								</div>
								<div class="flex flex-wrap gap-2">
									<Button variant="outline" :disabled="billingStore.loading[configForm.existingProvider.id]" @click="testBillingAccess">
										<RotateCw class="mr-2 h-4 w-4" />{{ store.getTranslation('settings.providers.billing.refresh') }}
									</Button>
									<Button v-if="billingForm.hasConnection" variant="destructive" @click="removeBilling">
										{{ store.getTranslation('settings.providers.billing.remove') }}
									</Button>
								</div>
							</div>
							<p v-else class="rounded-md bg-muted p-3 text-sm text-muted-foreground">
								{{ store.getTranslation('settings.providers.billing.local_only') }}
							</p>
						</div>
					</TabsContent>

					<TabsContent v-if="showCatalogTab" value="catalog">
						<div class="space-y-3 py-3">
							<Input v-model="catalogSearch" type="text" :placeholder="store.getTranslation('settings.providers.catalog_search')" />

							<div v-if="catalogLoading" class="flex items-center justify-center py-8 text-muted-foreground">
								<Loader2 class="h-5 w-5 animate-spin" />
							</div>
							<div v-else-if="catalogModels.length === 0" class="py-8 text-center text-sm text-muted-foreground">
								{{ store.getTranslation('settings.providers.catalog_empty') }}
							</div>
							<div v-else class="max-h-[320px] overflow-y-auto space-y-1.5">
								<div
									v-for="m in catalogModels"
									:key="m.id"
									class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2"
									:class="m.availability_state === 'USER_UNAVAILABLE' ? 'opacity-60' : ''"
								>
									<div class="min-w-0">
										<div class="text-sm font-medium text-foreground truncate">{{ m.display_name }}</div>
										<div class="text-xs text-muted-foreground truncate">{{ m.gateway_model_id }}</div>
									</div>
									<span
										v-if="m.availability_state === 'USER_UNAVAILABLE'"
										class="shrink-0 inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground"
									>
										{{ store.getTranslation('settings.providers.catalog_disabled_key') }}
									</span>
									<span v-else class="shrink-0 inline-flex items-center rounded-full bg-green-500/10 px-2 py-0.5 text-xs font-medium text-green-600">
										{{ store.getTranslation('settings.providers.catalog_available') }}
									</span>
								</div>
							</div>
						</div>
					</TabsContent>
				</Tabs>

				<DialogFooter class="gap-2 sm:gap-0">
					<Button v-if="configForm.existingProvider && activeProviderTab === 'settings'" variant="destructive" @click="deleteConfig" :disabled="saving" class="mr-auto">
						<Trash2 class="h-4 w-4 mr-2" />
						{{ store.getTranslation('common.delete') }}
					</Button>
					<div class="flex flex-row gap-2">
						<Button variant="outline" @click="dialogOpen = false">
							{{ store.getTranslation('common.cancel') }}
						</Button>
						<Button v-if="activeProviderTab !== 'catalog'" @click="saveActiveTab" :disabled="saving || (activeProviderTab === 'billing' && !supportsUpstreamBilling)">
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
import {ref, reactive, onMounted, computed, watch} from 'vue';
import {Sparkles, Cpu, Zap, Server, Plus, Settings2, Loader2, BrainCircuit, Globe, AudioWaveform, Trash2, RotateCw} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {useProviderBillingStore} from '@/stores/providerBillingStore';
import type {ProviderBillingOverview} from '@/types/providers';
import {Button} from '@/components/ui/button';
import {Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Input} from '@/components/ui/input';
import {Label} from '@/components/ui/label';
import {Switch} from '@/components/ui/switch';
import {Tabs, TabsContent, TabsList, TabsTrigger} from '@/components/ui/tabs';
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
const billingStore = useProviderBillingStore();
const dialogOpen = ref(false);
const selectedProvider = ref<ProviderConfig | null>(null);
const saving = ref(false);
const configuredProviders = ref<ConfiguredProvider[]>([]);

// Provider modal catalog tab (OpenRouter only): browse the gateway catalog, including
// models the configured key cannot run (USER_UNAVAILABLE).
const activeProviderTab = ref('settings');
const catalogModels = ref<any[]>([]);
const catalogLoading = ref(false);
const catalogLoaded = ref(false);
const catalogSearch = ref('');
let catalogDebounce: ReturnType<typeof setTimeout>;

const showCatalogTab = computed(() => configForm.existingProvider?.kind === 'OPENROUTER');
const selectedProviderKind = computed(() => configForm.existingProvider?.kind ?? selectedProvider.value?.kind);
const supportsUpstreamBilling = computed(() => selectedProviderKind.value === 'OPENAI' || selectedProviderKind.value === 'OPENROUTER');

const billingForm = reactive({
	isEnabled: false,
	credential: '',
	scopeId: '',
	scopeName: '',
	hasCredential: false,
	hasConnection: false,
});

async function loadCatalog() {
	if (!configForm.existingProvider) return;
	catalogLoading.value = true;
	try {
		const query = new URLSearchParams({page: '1', size: '50', query: catalogSearch.value});
		const res = await $customFetch<{has_more: boolean; items: any[]}>(`/api/v1/admin/providers/${configForm.existingProvider.id}/catalog?` + query.toString());
		catalogModels.value = res?.items || [];
		catalogLoaded.value = true;
	} catch (e) {
		console.error('Failed to load catalog:', e);
	} finally {
		catalogLoading.value = false;
	}
}

watch(activeProviderTab, tab => {
	if (tab === 'catalog' && !catalogLoaded.value) loadCatalog();
});

watch(catalogSearch, () => {
	if (activeProviderTab.value !== 'catalog') return;
	clearTimeout(catalogDebounce);
	catalogDebounce = setTimeout(() => loadCatalog(), 500);
});

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
			billing: billingStore.overviews[conf.id],
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

	// Reset the catalog tab for the newly opened provider.
	activeProviderTab.value = 'settings';
	catalogLoaded.value = false;
	catalogModels.value = [];
	catalogSearch.value = '';

	dialogOpen.value = true;
	if (item.isConfigured) loadBillingForm(item.id);
}

async function loadBillingForm(providerId: string) {
	try {
		const billing = await billingStore.fetchProviderBilling(providerId);
		billingForm.isEnabled = billing.is_enabled;
		billingForm.credential = '';
		billingForm.scopeId = billing.external_scope_id ?? '';
		billingForm.scopeName = billing.external_scope_name ?? '';
		billingForm.hasCredential = billing.has_billing_credential;
		billingForm.hasConnection = billing.is_enabled || billing.has_billing_credential || billing.external_scope_id !== null;
	} catch (error) {
		console.error('Failed to load billing configuration:', error);
	}
}

function billingStatusKey(billing: ProviderBillingOverview) {
	if ((billing.status === 'UPSTREAM_ERROR' || billing.status === 'UNAUTHORIZED') && billing.upstream) return 'settings.providers.billing.failed';
	if (billing.status === 'AVAILABLE' && billing.upstream) return 'settings.providers.billing.provider_reported';
	if (billing.status === 'UNSUPPORTED') return 'settings.providers.billing.unsupported';
	if (billing.status === 'NOT_CONFIGURED' || billing.status === 'UNAUTHORIZED') return 'settings.providers.billing.setup_required';
	return 'settings.providers.billing.failed';
}

function decimalValue(value: string | number | null | undefined) {
	const parsed = Number(value ?? 0);
	return Number.isFinite(parsed) ? parsed : 0;
}

function formatMoney(value: string | number | null | undefined, currency: string) {
	return new Intl.NumberFormat(undefined, {style: 'currency', currency: currency || 'USD'}).format(decimalValue(value));
}

function upstreamSummary(billing: ProviderBillingOverview) {
	const metric = billing.upstream;
	if (!metric) return '';
	if (metric.metric_kind === 'CREDIT_BALANCE') {
		return store.getTranslation('settings.providers.billing.credits_remaining', {amount: formatMoney(metric.remaining_amount, metric.currency)});
	}
	return store.getTranslation('settings.providers.billing.spent_month', {amount: formatMoney(metric.spent_amount, metric.currency)});
}

function upstreamDetail(billing: ProviderBillingOverview) {
	const metric = billing.upstream;
	if (!metric || metric.limit_amount == null) return '';
	if (metric.metric_kind === 'KEY_LIMIT') {
		return store.getTranslation('settings.providers.billing.key_limit', {amount: formatMoney(metric.limit_amount, metric.currency)});
	}
	if (metric.metric_kind === 'SPEND_THRESHOLD' && decimalValue(metric.spent_amount) > decimalValue(metric.limit_amount)) {
		return store.getTranslation('settings.providers.billing.over_threshold', {
			amount: formatMoney(decimalValue(metric.spent_amount) - decimalValue(metric.limit_amount), metric.currency),
		});
	}
	if (metric.metric_kind === 'SPEND_THRESHOLD') {
		return store.getTranslation('settings.providers.billing.remaining_threshold', {
			amount: formatMoney(metric.remaining_amount, metric.currency),
			threshold: formatMoney(metric.limit_amount, metric.currency),
		});
	}
	return '';
}

function billingProgress(billing: ProviderBillingOverview) {
	const metric = billing.upstream;
	const limit = decimalValue(metric?.limit_amount);
	if (!metric || limit <= 0) return 0;
	return Math.min(100, Math.max(0, (decimalValue(metric.spent_amount) / limit) * 100));
}

function updatedLabel(value: string | null) {
	if (!value) return store.getTranslation('settings.providers.billing.never_updated');
	return store.getTranslation('settings.providers.billing.updated', {time: store.formatDate(value, 'L LT')});
}

async function refreshBilling(providerId: string) {
	try {
		const billing = await billingStore.refreshProviderBilling(providerId);
		if (billing.status === 'UPSTREAM_ERROR' || billing.status === 'UNAUTHORIZED' || billing.status === 'NOT_CONFIGURED') {
			store.toast(store.getTranslation('settings.providers.billing.refresh_failed'), {type: 'error'});
		}
	} catch (error: any) {
		store.toast(store.getTranslation('settings.providers.billing.refresh_failed'), {type: 'error', description: error?.message});
	}
}

async function saveBilling() {
	const provider = configForm.existingProvider;
	if (!provider) return false;
	saving.value = true;
	try {
		const suppliedCredential = billingForm.credential.length > 0;
		await billingStore.updateProviderBilling(provider.id, {
			is_enabled: billingForm.isEnabled,
			...(billingForm.credential ? {credential: billingForm.credential} : {}),
			external_scope_id: billingForm.scopeId || null,
			external_scope_name: billingForm.scopeName || null,
		});
		billingForm.credential = '';
		billingForm.hasCredential = billingForm.hasCredential || suppliedCredential;
		billingForm.hasConnection = true;
		store.toast(store.getTranslation('settings.providers.billing.saved'), {type: 'success'});
		return true;
	} catch (error: any) {
		store.toast(store.getTranslation('settings.providers.save_error'), {type: 'error', description: error?.message});
		return false;
	} finally {
		saving.value = false;
	}
}

async function testBillingAccess() {
	const saved = await saveBilling();
	if (saved && billingForm.isEnabled && configForm.existingProvider) await refreshBilling(configForm.existingProvider.id);
}

async function removeBilling() {
	if (!configForm.existingProvider) return;
	try {
		await billingStore.removeProviderBilling(configForm.existingProvider.id);
		await billingStore.fetchBillingOverviews();
		billingForm.isEnabled = false;
		billingForm.credential = '';
		billingForm.scopeId = '';
		billingForm.scopeName = '';
		billingForm.hasCredential = false;
		billingForm.hasConnection = false;
		store.toast(store.getTranslation('settings.providers.billing.removed'), {type: 'success'});
	} catch (error: any) {
		store.toast(store.getTranslation('settings.providers.save_error'), {type: 'error', description: error?.message});
	}
}

function saveActiveTab() {
	if (activeProviderTab.value === 'billing') return saveBilling();
	return saveConfig();
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
	Promise.all([loadProviders(), billingStore.fetchBillingOverviews()]);
});
</script>
