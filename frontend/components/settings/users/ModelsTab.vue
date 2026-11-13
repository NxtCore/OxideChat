<template>
	<SettingsUsersSection
		:title="t('models_title')"
		:description="t('models_description', {usable: access.usable, total: access.total})"
		to="/settings/teams"
		:link-label="t('manage_teams')"
		:empty-title="access.grants.length ? undefined : t('models_empty')"
		:empty-hint="t('models_empty_hint')"
		test-id="user-models"
	>
		<ul class="divide-y divide-border overflow-hidden rounded-lg border border-border bg-card">
			<li v-for="grant in access.grants" :key="`${grant.team.id}:${grant.kind}:${grant.target?.id ?? ''}`">
				<NuxtLink
					:to="linkOf(grant)"
					class="flex items-center gap-4 px-4 py-3 transition-colors hover:bg-accent/40 focus-visible:bg-accent/40 focus-visible:outline-none"
					data-testid="user-model-grant"
					:data-kind="grant.kind"
				>
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-primary/10">
						<component :is="ICONS[grant.kind]" class="h-4 w-4 text-primary" />
					</span>
					<span class="min-w-0 flex-1">
						<span class="block truncate text-sm font-medium text-foreground">{{ titleOf(grant) }}</span>
						<span class="block truncate text-xs text-muted-foreground">{{ t('grant_through', {team: grant.team.name}) }}</span>
					</span>
					<ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground" />
				</NuxtLink>
			</li>
		</ul>
	</SettingsUsersSection>
</template>

<script setup lang="ts">
import {Bot, ChevronRight, Cpu, Layers} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {ADMIN_PROVIDERS_VIEW} from '~/types/generated/permissions';
import type {UserModelAccessResponse} from '~/types/generated/UserModelAccessResponse';
import type {UserModelGrant} from '~/types/users';

defineProps<{access: UserModelAccessResponse}>();

const ICONS = {all: Layers, provider: Cpu, model: Bot};

const store = useMainStore();
const t = (key: string, args?: Record<string, string | number>) => store.getTranslation(`settings.user_detail.${key}`, args);

function titleOf(grant: UserModelGrant): string {
	if (grant.kind === 'all') return t('grant_all');
	const name = grant.target?.name ?? '';
	return grant.kind === 'provider' ? t('grant_provider', {name}) : name;
}

function linkOf(grant: UserModelGrant): string {
	if (grant.kind === 'model' && grant.target && store.hasPermission(ADMIN_PROVIDERS_VIEW)) return `/settings/models/${grant.target.id}`;
	return `/settings/teams?team=${grant.team.id}`;
}
</script>
