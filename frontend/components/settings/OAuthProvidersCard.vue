<template>
	<div v-if="loaded" class="mt-6 rounded-lg border border-border bg-card p-6" data-testid="oauth-card">
		<div class="mb-4 flex items-start justify-between gap-4">
			<div>
				<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.admin_config.oauth.title') }}</h3>
				<p class="mt-0.5 text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.description') }}</p>
			</div>
			<ShadButton v-if="canEdit" size="sm" data-testid="oauth-add" @click="openCreate">
				{{ store.getTranslation('settings.admin_config.oauth.add') }}
			</ShadButton>
		</div>

		<p v-if="providers.length === 0" class="text-sm text-muted-foreground" data-testid="oauth-empty">{{ store.getTranslation('settings.admin_config.oauth.empty') }}</p>

		<SettingsRow v-for="item in providers" :key="item.id" :label="item.name" :hint="item.enabled ? item.slug : `${item.slug} · ${store.getTranslation('settings.admin_config.oauth.disabled')}`">
			<ShadButton variant="outline" size="sm" :disabled="!canEdit" :data-testid="`oauth-configure-${item.slug}`" @click="openEdit(item)">
				{{ store.getTranslation('settings.admin_config.oauth.configure') }}
			</ShadButton>
		</SettingsRow>

		<ShadDialog v-model:open="dialogOpen">
			<ShadDialogContent class="max-h-[90vh] overflow-y-auto sm:max-w-[560px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ editing ? editing.name : store.getTranslation('settings.admin_config.oauth.add') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('settings.admin_config.oauth.description') }}</ShadDialogDescription>
				</ShadDialogHeader>

				<div class="space-y-4 py-2">
					<div v-if="!editing" class="space-y-2">
						<ShadLabel>{{ store.getTranslation('settings.admin_config.oauth.start_from') }}</ShadLabel>
						<div class="flex flex-wrap gap-2">
							<ShadButton variant="outline" size="sm" type="button" data-testid="oauth-preset-custom" @click="applyPreset(null)">
								{{ store.getTranslation('settings.admin_config.oauth.custom') }}
							</ShadButton>
							<ShadButton v-for="preset in presets" :key="preset.slug" variant="outline" size="sm" type="button" :data-testid="`oauth-preset-${preset.slug}`" @click="applyPreset(preset)">
								{{ preset.name }}
							</ShadButton>
						</div>
					</div>

					<div class="grid gap-4 sm:grid-cols-2">
						<div class="space-y-2">
							<ShadLabel for="oauth-name">{{ store.getTranslation('settings.admin_config.oauth.name') }}</ShadLabel>
							<ShadInput id="oauth-name" v-model="form.name" data-testid="oauth-name" autocomplete="off" />
						</div>
						<div class="space-y-2">
							<ShadLabel for="oauth-slug">{{ store.getTranslation('settings.admin_config.oauth.slug') }}</ShadLabel>
							<ShadInput id="oauth-slug" v-model="form.slug" :disabled="Boolean(editing)" data-testid="oauth-slug" autocomplete="off" />
						</div>
					</div>
					<p v-if="!editing" class="-mt-2 text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.slug_hint') }}</p>

					<div class="space-y-2">
						<ShadLabel for="oauth-issuer">{{ store.getTranslation('settings.admin_config.oauth.issuer_url') }}</ShadLabel>
						<ShadInput id="oauth-issuer" v-model="form.issuer_url" data-testid="oauth-issuer" placeholder="https://id.example.com/realms/main" autocomplete="off" />
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.issuer_hint') }}</p>
					</div>

					<div class="grid gap-4 sm:grid-cols-2">
						<div class="space-y-2">
							<ShadLabel for="oauth-client-id">{{ store.getTranslation('settings.admin_config.oauth.client_id') }}</ShadLabel>
							<ShadInput id="oauth-client-id" v-model="form.client_id" data-testid="oauth-client-id" autocomplete="off" />
						</div>
						<div class="space-y-2">
							<ShadLabel for="oauth-client-secret">{{ store.getTranslation('settings.admin_config.oauth.client_secret') }}</ShadLabel>
							<ShadInput
								id="oauth-client-secret"
								v-model="form.client_secret"
								type="password"
								data-testid="oauth-client-secret"
								autocomplete="new-password"
								:placeholder="editing ? '••••••••' : ''"
							/>
						</div>
					</div>
					<p class="-mt-2 text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.secret_hint') }}</p>

					<div v-if="callbackUrl" class="space-y-1">
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.callback_hint') }}</p>
						<code class="block break-all rounded-md bg-muted p-2 text-xs" data-testid="oauth-callback-url">{{ callbackUrl }}</code>
					</div>

					<div class="space-y-2">
						<ShadLabel for="oauth-scopes">{{ store.getTranslation('settings.admin_config.oauth.scopes') }}</ShadLabel>
						<ShadInput id="oauth-scopes" v-model="form.scopes" data-testid="oauth-scopes" autocomplete="off" />
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.scopes_hint') }}</p>
					</div>

					<details class="rounded-md border border-border p-3" data-testid="oauth-advanced">
						<summary class="cursor-pointer text-sm font-medium">{{ store.getTranslation('settings.admin_config.oauth.advanced') }}</summary>
						<div class="mt-3 space-y-3">
							<div v-for="field in endpointFields" :key="field" class="space-y-1">
								<ShadLabel :for="`oauth-${field}`">{{ store.getTranslation(`settings.admin_config.oauth.${field}`) }}</ShadLabel>
								<ShadInput :id="`oauth-${field}`" v-model="form[field]" :data-testid="`oauth-${field}`" autocomplete="off" />
							</div>
							<p class="pt-2 text-xs text-muted-foreground">{{ store.getTranslation('settings.admin_config.oauth.claims_hint') }}</p>
							<div class="grid gap-3 sm:grid-cols-2">
								<div v-for="field in claimFields" :key="field" class="space-y-1">
									<ShadLabel :for="`oauth-claim-${field}`">{{ store.getTranslation(`settings.admin_config.oauth.claim_${field}`) }}</ShadLabel>
									<ShadInput :id="`oauth-claim-${field}`" v-model="form.claims[field]" :data-testid="`oauth-claim-${field}`" autocomplete="off" />
								</div>
							</div>
						</div>
					</details>

					<SettingsRow :label="store.getTranslation('settings.admin_config.oauth.enabled')">
						<ShadSwitch v-model="form.enabled" data-testid="oauth-enabled" />
					</SettingsRow>
					<SettingsRow :label="store.getTranslation('settings.admin_config.oauth.allow_registration')" :hint="store.getTranslation('settings.admin_config.oauth.allow_registration_hint')">
						<ShadSwitch v-model="form.allow_registration" data-testid="oauth-allow-registration" />
					</SettingsRow>
					<SettingsRow :label="store.getTranslation('settings.admin_config.oauth.trust_email')" :hint="store.getTranslation('settings.admin_config.oauth.trust_email_hint')">
						<ShadSwitch v-model="form.trust_email" data-testid="oauth-trust-email" />
					</SettingsRow>
				</div>

				<ShadDialogFooter class="gap-2 sm:gap-0">
					<ShadButton v-if="editing" variant="destructive" class="mr-auto" :disabled="saving" data-testid="oauth-remove" @click="remove">
						{{ store.getTranslation('common.delete') }}
					</ShadButton>
					<div class="flex flex-row gap-2">
						<ShadButton variant="outline" @click="dialogOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
						<ShadButton :disabled="!canSave || saving" data-testid="oauth-save" @click="save">
							{{ store.getTranslation('common.save') }}
						</ShadButton>
					</div>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {computed, onMounted, reactive, ref} from 'vue';
import {useMainStore} from '@/stores';
import {ADMIN_CONFIG_EDIT} from '~/types/generated/permissions';
import type {OAuthPresetResponse} from '~/types/generated/OAuthPresetResponse';
import type {OAuthProviderResponse} from '~/types/generated/OAuthProviderResponse';
import type {ClaimMapping} from '~/types/generated/ClaimMapping';

type EndpointField = 'authorization_url' | 'token_url' | 'userinfo_url';
type ClaimField = 'subject' | 'email' | 'email_verified' | 'name';

const endpointFields: EndpointField[] = ['authorization_url', 'token_url', 'userinfo_url'];
const claimFields: ClaimField[] = ['subject', 'email', 'email_verified', 'name'];
const DEFAULT_CLAIMS: ClaimMapping = {subject: 'sub', email: 'email', email_verified: 'email_verified', name: 'name', avatar: 'picture', groups: 'groups'};

const store = useMainStore();
const canEdit = computed(() => store.hasPermission(ADMIN_CONFIG_EDIT));

const loaded = ref(false);
const providers = ref<OAuthProviderResponse[]>([]);
const presets = ref<OAuthPresetResponse[]>([]);
const editing = ref<OAuthProviderResponse | null>(null);
const dialogOpen = ref(false);
const saving = ref(false);

const form = reactive({
	slug: '',
	name: '',
	icon: null as string | null,
	issuer_url: '',
	authorization_url: '',
	token_url: '',
	userinfo_url: '',
	client_id: '',
	client_secret: '',
	scopes: '',
	claims: {...DEFAULT_CLAIMS} as ClaimMapping,
	trust_email: false,
	allow_registration: true,
	enabled: true,
});

const hasEndpoints = computed(() => form.issuer_url.trim() !== '' || (form.authorization_url.trim() !== '' && form.token_url.trim() !== '' && form.userinfo_url.trim() !== ''));
const canSave = computed(
	() =>
		form.name.trim() !== '' &&
		form.client_id.trim() !== '' &&
		hasEndpoints.value &&
		(editing.value !== null || (/^[a-z0-9][a-z0-9-]{0,49}$/.test(form.slug) && form.client_secret !== '')),
);
const callbackUrl = computed(() => {
	if (editing.value) return editing.value.callback_url;
	const base = window.location.origin;
	return form.slug ? `${base}/api/v1/auth/oauth/${form.slug}/callback` : null;
});

function fill(source: {name: string; slug: string; icon: string | null; endpoints: OAuthPresetResponse['endpoints']; scopes: string[]; claim_mapping: ClaimMapping}) {
	form.name = source.name;
	form.slug = source.slug;
	form.icon = source.icon;
	form.issuer_url = source.endpoints.issuer_url ?? '';
	form.authorization_url = source.endpoints.authorization_url ?? '';
	form.token_url = source.endpoints.token_url ?? '';
	form.userinfo_url = source.endpoints.userinfo_url ?? '';
	form.scopes = source.scopes.join(' ');
	form.claims = {...source.claim_mapping};
}

function applyPreset(preset: OAuthPresetResponse | null) {
	const keep = {client_id: form.client_id, client_secret: form.client_secret};
	fill(preset ?? {name: '', slug: '', icon: null, endpoints: {issuer_url: null, authorization_url: null, token_url: null, userinfo_url: null}, scopes: ['openid', 'email', 'profile'], claim_mapping: DEFAULT_CLAIMS});
	Object.assign(form, keep);
}

function openCreate() {
	editing.value = null;
	Object.assign(form, {client_id: '', client_secret: '', trust_email: false, allow_registration: true, enabled: true});
	applyPreset(null);
	dialogOpen.value = true;
}

function openEdit(item: OAuthProviderResponse) {
	editing.value = item;
	fill(item);
	Object.assign(form, {client_id: item.client_id, client_secret: '', trust_email: item.trust_email, allow_registration: item.allow_registration, enabled: item.enabled});
	dialogOpen.value = true;
}

function orNull(value: string): string | null {
	return value.trim() === '' ? null : value.trim();
}

function requestBody() {
	return {
		slug: form.slug,
		name: form.name.trim(),
		icon: form.icon,
		endpoints: {issuer_url: orNull(form.issuer_url), authorization_url: orNull(form.authorization_url), token_url: orNull(form.token_url), userinfo_url: orNull(form.userinfo_url)},
		client_id: form.client_id.trim(),
		client_secret: form.client_secret || null,
		scopes: form.scopes.split(/[\s,]+/).filter(Boolean),
		claim_mapping: form.claims,
		trust_email: form.trust_email,
		allow_registration: form.allow_registration,
		enabled: form.enabled,
	};
}

async function load() {
	[providers.value, presets.value] = await Promise.all([oauthProviders<OAuthProviderResponse>(), oauthPresets<OAuthPresetResponse>()]);
	loaded.value = true;
}

async function run(action: () => Promise<unknown>) {
	saving.value = true;
	try {
		await action();
		await load();
		dialogOpen.value = false;
		store.toast(store.getTranslation('settings.admin_config.save_success'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('settings.admin_config.save_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}

const save = () => run(() => (editing.value ? updateOauthProvider(editing.value.id, requestBody()) : createOauthProvider(requestBody())));
const remove = () => run(() => deleteOauthProvider(editing.value!.id));

onMounted(() => load().catch(() => {}));
</script>
