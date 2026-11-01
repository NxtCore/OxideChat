<template>
	<SettingsPage :title="store.getTranslation('settings.sessions.title')" :description="store.getTranslation('settings.sessions.description')" layout="form">
		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>
		<div v-else class="space-y-3">
			<div v-for="session in sessions" :key="session.id" class="flex items-center justify-between gap-4 rounded-lg border border-border bg-card p-4" data-testid="session-row">
				<div class="min-w-0">
					<div class="flex items-center gap-2">
						<p class="text-sm font-medium text-foreground">{{ store.getTranslation(session.is_current ? 'settings.sessions.current' : 'settings.sessions.other') }}</p>
					</div>
					<p class="mt-0.5 text-xs text-muted-foreground">
						{{ store.getTranslation('settings.sessions.created') }}: {{ store.formatDate(session.created_at, 'L LT') }} ·
						{{ store.getTranslation('settings.sessions.expires') }}: {{ store.formatDate(session.expires_at, 'L LT') }}
					</p>
				</div>
				<ShadButton v-if="!session.is_current" variant="outline" size="sm" :disabled="busyId === session.id" data-testid="session-sign-out" @click="signOut(session)">
					{{ store.getTranslation('settings.sessions.sign_out') }}
				</ShadButton>
			</div>
		</div>
	</SettingsPage>
</template>

<script setup lang="ts">
import {onMounted, ref} from 'vue';
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import type {SessionResponse} from '~/types/generated/SessionResponse';

const store = useMainStore();

const sessions = ref<SessionResponse[]>([]);
const loading = ref(true);
const busyId = ref<string | null>(null);

async function signOut(session: SessionResponse) {
	busyId.value = session.id;
	try {
		await revokeMySession(session.id);
		sessions.value = sessions.value.filter(item => item.id !== session.id);
		store.toast(store.getTranslation('settings.sessions.signed_out'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('settings.sessions.sign_out_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		busyId.value = null;
	}
}

onMounted(async () => {
	try {
		sessions.value = await mySessions<SessionResponse[]>();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.sessions.load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
});
</script>
