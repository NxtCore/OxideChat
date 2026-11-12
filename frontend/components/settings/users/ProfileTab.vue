<template>
	<div class="space-y-8">
		<SettingsUsersSection :title="t('account_title')" :description="t('account_description')" test-id="user-account">
			<div class="grid gap-4 md:grid-cols-2">
				<div class="space-y-1.5">
					<ShadLabel for="user-email">{{ store.getTranslation('settings.admin_users.field_email') }}</ShadLabel>
					<ShadInput id="user-email" v-model="form.email" type="email" autocomplete="off" :disabled="!overview.can_edit" data-testid="user-email" />
				</div>
				<div class="space-y-1.5">
					<ShadLabel for="user-username">{{ store.getTranslation('settings.admin_users.field_username') }}</ShadLabel>
					<ShadInput id="user-username" v-model="form.username" autocomplete="off" :disabled="!overview.can_edit" data-testid="user-username" />
				</div>
			</div>

			<dl class="divide-y divide-border rounded-lg border border-border text-sm">
				<div class="flex items-center justify-between gap-4 px-3 py-2.5">
					<dt class="text-muted-foreground">{{ t('auth_method') }}</dt>
					<dd class="text-foreground" data-testid="user-auth-method">{{ isLocal ? t('auth_local') : overview.profile.auth_method }}</dd>
				</div>
				<div class="flex items-center justify-between gap-4 px-3 py-2.5">
					<dt class="text-muted-foreground">{{ t('created') }}</dt>
					<dd class="text-foreground">{{ store.formatDate(overview.profile.created_at, 'L LT') }}</dd>
				</div>
				<div class="flex items-center justify-between gap-4 px-3 py-2.5">
					<dt class="shrink-0 text-muted-foreground">{{ store.getTranslation('settings.profile.user_id') }}</dt>
					<dd class="flex min-w-0 items-center gap-2">
						<span class="truncate font-mono text-xs text-foreground">{{ overview.profile.id }}</span>
						<ShadButton variant="ghost" size="icon" class="h-7 w-7 shrink-0" :aria-label="t('copy_id')" @click="store.copyToClipboard(overview.profile.id)">
							<Copy class="h-3.5 w-3.5" />
						</ShadButton>
					</dd>
				</div>
			</dl>

			<div class="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border p-3">
				<div class="min-w-0 flex-1 basis-64 space-y-0.5">
					<p class="text-sm font-medium text-foreground">{{ t('password_title') }}</p>
					<p class="text-xs text-muted-foreground">{{ isLocal ? t('password_hint') : t('password_external', {method: overview.profile.auth_method}) }}</p>
				</div>
				<ShadButton v-if="isLocal && overview.can_edit" variant="outline" size="sm" class="shrink-0 gap-2" data-testid="user-reset-password" @click="openPassword">
					<KeyRound class="h-4 w-4" />
					{{ store.getTranslation('settings.admin_users.reset_password') }}
				</ShadButton>
			</div>
		</SettingsUsersSection>

		<SettingsUsersSection
			:title="t('sessions_title')"
			:description="t('sessions_description')"
			:empty-title="overview.sessions.length ? undefined : t('sessions_empty')"
			:empty-hint="t('sessions_empty_hint')"
			test-id="user-sessions"
		>
			<ul class="divide-y divide-border rounded-lg border border-border bg-card">
				<li v-for="session in overview.sessions" :key="session.id" class="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3" data-testid="user-session">
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-muted">
						<MonitorSmartphone class="h-4 w-4 text-muted-foreground" />
					</span>
					<div class="min-w-0 flex-1 basis-56">
						<p class="text-sm font-medium text-foreground">{{ t(session.is_current ? 'session_yours' : 'session') }}</p>
						<p class="text-xs text-muted-foreground">
							{{ store.getTranslation('settings.sessions.created') }}: {{ store.formatDate(session.created_at, 'L LT') }} ·
							{{ store.getTranslation('settings.sessions.expires') }}: {{ store.formatDate(session.expires_at, 'L LT') }}
						</p>
					</div>
					<ShadButton
						v-if="overview.can_edit && !session.is_current"
						variant="outline"
						size="sm"
						class="shrink-0"
						:disabled="busySession === session.id"
						data-testid="user-session-sign-out"
						@click="signOut(session)"
					>
						{{ store.getTranslation('settings.sessions.sign_out') }}
					</ShadButton>
				</li>
			</ul>
		</SettingsUsersSection>

		<ShadDialog v-model:open="passwordOpen">
			<ShadDialogContent class="sm:max-w-[420px]">
				<form class="space-y-4" @submit.prevent="savePassword">
					<ShadDialogHeader>
						<ShadDialogTitle>{{ store.getTranslation('settings.admin_users.reset_password') }}</ShadDialogTitle>
						<ShadDialogDescription>{{ t('password_dialog_description', {username: overview.profile.username}) }}</ShadDialogDescription>
					</ShadDialogHeader>
					<div class="space-y-1.5">
						<ShadLabel for="user-new-password">{{ t('new_password') }}</ShadLabel>
						<ShadInput id="user-new-password" v-model="password" type="password" autocomplete="new-password" data-testid="user-new-password" />
					</div>
					<ShadDialogFooter>
						<ShadButton type="button" variant="outline" @click="passwordOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
						<ShadButton type="submit" :disabled="savingPassword || !password" data-testid="user-password-save">
							<Loader2 v-if="savingPassword" class="h-4 w-4 animate-spin" />
							{{ store.getTranslation('common.save') }}
						</ShadButton>
					</ShadDialogFooter>
				</form>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {Copy, KeyRound, Loader2, MonitorSmartphone} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {SessionResponse} from '~/types/generated/SessionResponse';
import type {UserOverviewResponse} from '~/types/users';

const props = defineProps<{overview: UserOverviewResponse; form: {email: string; username: string}}>();
const emit = defineEmits<{changed: []}>();

const store = useMainStore();
const t = (key: string, args?: Record<string, string>) => store.getTranslation(`settings.user_detail.${key}`, args);

const isLocal = computed(() => props.overview.profile.auth_method === 'local');
const busySession = ref<string | null>(null);
const passwordOpen = ref(false);
const password = ref('');
const savingPassword = ref(false);

function openPassword() {
	password.value = '';
	passwordOpen.value = true;
}

async function savePassword() {
	savingPassword.value = true;
	try {
		await updateAdminUser(props.overview.profile.id, {password: password.value}, '/password');
		store.toast(t('password_success'), {type: 'success'});
		passwordOpen.value = false;
	} catch (e: any) {
		store.toast(t('password_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		savingPassword.value = false;
	}
}

async function signOut(session: SessionResponse) {
	busySession.value = session.id;
	try {
		await revokeAdminUserSession(props.overview.profile.id, session.id);
		store.toast(store.getTranslation('settings.sessions.signed_out'), {type: 'success'});
		emit('changed');
	} catch (e: any) {
		store.toast(store.getTranslation('settings.sessions.sign_out_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		busySession.value = null;
	}
}
</script>
