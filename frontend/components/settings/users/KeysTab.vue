<template>
	<SettingsUsersSection
		:title="t('keys_title')"
		:description="t('keys_description')"
		to="/settings/gateway-admin"
		:link-label="t('manage_keys')"
		:empty-title="keys.length ? undefined : t('keys_empty')"
		:empty-hint="t('keys_empty_hint')"
		test-id="user-keys"
	>
		<ul class="divide-y divide-border rounded-lg border border-border bg-card">
			<li v-for="apiKey in keys" :key="apiKey.id" class="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3" data-testid="user-key">
				<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-primary/10">
					<KeyRound class="h-4 w-4 text-primary" />
				</span>
				<div class="min-w-0 flex-1 basis-56">
					<div class="flex flex-wrap items-center gap-2">
						<span class="truncate text-sm font-medium text-foreground">{{ apiKey.name }}</span>
						<ShadBadge :variant="apiKey.status === 'active' ? 'default' : apiKey.status === 'revoked' ? 'destructive' : 'secondary'">
							{{ store.getTranslation(`developer.status.${apiKey.status}`) }}
						</ShadBadge>
					</div>
					<p class="truncate text-xs text-muted-foreground">
						<span class="font-mono">{{ apiKey.key_prefix }}…{{ apiKey.last_four }}</span> · {{ apiKey.team_name }} ·
						{{ apiKey.last_used_at ? t('key_last_used', {date: formatDateTime(apiKey.last_used_at)}) : t('key_never_used') }}
					</p>
				</div>
				<ShadButton
					variant="ghost"
					size="icon"
					class="h-8 w-8 shrink-0"
					:aria-label="store.getTranslation('developer.admin.keys.inspect')"
					data-testid="user-key-inspect"
					@click="inspectId = apiKey.id"
				>
					<Eye class="h-4 w-4" />
				</ShadButton>
			</li>
		</ul>
		<DeveloperAdminKeyDetailDialog :open="!!inspectId" :key-id="inspectId" @update:open="inspectId = null" />
	</SettingsUsersSection>
</template>

<script setup lang="ts">
import {Eye, KeyRound} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AdminKey} from '~/types/gatewayAdmin';

defineProps<{keys: AdminKey[]}>();

const store = useMainStore();
const t = (key: string, args?: Record<string, string>) => store.getTranslation(`settings.user_detail.${key}`, args);

const inspectId = ref<string | null>(null);
</script>
