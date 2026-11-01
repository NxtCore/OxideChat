<template>
	<div class="space-y-6">
		<div>
			<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.admin.title') }}</h2>
			<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.admin.description') }}</p>
		</div>

		<div class="flex gap-1 overflow-x-auto border-b border-border">
			<button
				v-for="section in visibleSections"
				:key="section.id"
				type="button"
				class="shrink-0 border-b-2 px-3 py-2 text-sm transition-colors"
				:class="active === section.id ? 'border-primary font-medium text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'"
				@click="active = section.id"
			>
				{{ store.getTranslation(section.label) }}
			</button>
		</div>

		<DeveloperAdminKeys v-if="active === 'keys'" />
		<DeveloperAdminTeams v-else-if="active === 'teams'" />
		<DeveloperAdminAudit v-else-if="active === 'audit'" />
		<DeveloperAdminEvents v-else-if="active === 'events'" />
	</div>
</template>

<script setup lang="ts">
import {ADMIN_DEVELOPER_AUDIT_VIEW, ADMIN_DEVELOPER_VIEW} from '~/types/generated/permissions';
import {useMainStore} from '@/stores';

const store = useMainStore();

const sections = [
	{id: 'keys', label: 'developer.admin.tabs.keys', permission: ADMIN_DEVELOPER_VIEW},
	{id: 'teams', label: 'developer.admin.tabs.teams', permission: ADMIN_DEVELOPER_VIEW},
	{id: 'audit', label: 'developer.admin.tabs.audit', permission: ADMIN_DEVELOPER_AUDIT_VIEW},
	{id: 'events', label: 'developer.admin.tabs.events', permission: ADMIN_DEVELOPER_VIEW},
];

const visibleSections = computed(() => sections.filter(section => store.hasPermission(section.permission)));
const active = ref(visibleSections.value[0]?.id ?? 'keys');
</script>
