<template>
	<div v-if="providers.length > 0" class="mt-6 rounded-lg border border-border bg-card p-6" data-testid="oauth-card">
		<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.admin_config.oauth.title') }}</h3>
		<p class="mt-0.5 mb-4 text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.description') }}</p>

		<SettingsRow v-for="item in providers" :key="item.provider" :label="providerName(item.provider)" :hint="statusLabel(item)">
			<ShadButton variant="outline" size="sm" :disabled="!canEdit" :data-testid="`oauth-configure-${item.provider}`" @click="openDialog(item)">
				{{ store.getTranslation('settings.admin_config.oauth.configure') }}
			</ShadButton>
		</SettingsRow>

		<ShadDialog v-model:open="dialogOpen">
			<ShadDialogContent class="sm:max-w-[500px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ selected ? providerName(selected.provider) : '' }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('settings.admin_config.oauth.description') }}</ShadDialogDescription>
				</ShadDialogHeader>

				<div class="space-y-4 py-2">
					<div class="space-y-2">
						<ShadLabel for="oauth-client-id">{{ store.getTranslation('settings.admin_config.oauth.client_id') }}</ShadLabel>
						<ShadInput id="oauth-client-id" v-model="clientId" data-testid="oauth-client-id" autocomplete="off" />
					</div>
					<div class="space-y-2">
						<ShadLabel for="oauth-client-secret">{{ store.getTranslation('settings.admin_config.oauth.client_secret') }}</ShadLabel>
						<ShadInput id="oauth-client-secret" v-model="clientSecret" type="password" data-testid="oauth-client-secret" autocomplete="new-password" :placeholder="selected?.has_client_secret ? '••••••••' : ''" />
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.secret_hint') }}</p>
					</div>
					<div v-if="selected?.callback_url" class="space-y-1">
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.callback_hint') }}</p>
						<code class="block break-all rounded-md bg-muted p-2 text-xs">{{ selected.callback_url }}</code>
					</div>
				</div>

				<ShadDialogFooter class="gap-2 sm:gap-0">
					<ShadButton v-if="hasSavedValues" variant="destructive" class="mr-auto" :disabled="saving" data-testid="oauth-remove" @click="save(null, null)">
						{{ store.getTranslation('settings.admin_config.oauth.remove') }}
					</ShadButton>
					<div class="flex flex-row gap-2">
						<ShadButton variant="outline" @click="dialogOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
						<ShadButton :disabled="!canSave || saving" data-testid="oauth-save" @click="save(clientId.trim(), clientSecret || undefined)">
							{{ store.getTranslation('common.save') }}
						</ShadButton>
					</div>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {computed, onMounted, ref} from 'vue';
import {useMainStore} from '@/stores';
import {ADMIN_CONFIG_EDIT} from '~/types/generated/permissions';
import type {AdminConfigResponse} from '~/types/generated/AdminConfigResponse';
import type {OAuthSettingsResponse} from '~/types/generated/OAuthSettingsResponse';

const store = useMainStore();
const canEdit = computed(() => store.hasPermission(ADMIN_CONFIG_EDIT));

const providers = ref<OAuthSettingsResponse[]>([]);
const selected = ref<OAuthSettingsResponse | null>(null);
const dialogOpen = ref(false);
const saving = ref(false);
const clientId = ref('');
const clientSecret = ref('');

const hasSavedValues = computed(() => Boolean(selected.value?.client_id || selected.value?.has_client_secret));
const canSave = computed(() => clientId.value.trim() !== '' && (clientSecret.value !== '' || Boolean(selected.value?.has_client_secret)));

function providerName(provider: string): string {
	return provider.charAt(0) + provider.slice(1).toLowerCase();
}

function statusLabel(item: OAuthSettingsResponse): string {
	if (!item.configured) return store.getTranslation('settings.admin_config.oauth.not_configured');
	const configured = store.getTranslation('settings.admin_config.oauth.configured');
	return item.client_id ? configured : `${configured} (${store.getTranslation('settings.admin_config.oauth.from_env')})`;
}

async function load() {
	const config = await adminConfig<AdminConfigResponse>();
	providers.value = config.oauth;
	if (store.base) store.base.oauth_providers = config.oauth.filter(item => item.configured).map(item => item.provider);
}

function openDialog(item: OAuthSettingsResponse) {
	selected.value = item;
	clientId.value = item.client_id ?? '';
	clientSecret.value = '';
	dialogOpen.value = true;
}

async function save(id: string | null, secret: string | null | undefined) {
	if (!selected.value) return;
	saving.value = true;
	try {
		await patchAdminConfig({oauth: [{provider: selected.value.provider, client_id: id, ...(secret === undefined ? {} : {client_secret: secret})}]});
		await load();
		dialogOpen.value = false;
		store.toast(store.getTranslation('settings.admin_config.save_success'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('settings.admin_config.save_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}

onMounted(() => load().catch(() => {}));
</script>
