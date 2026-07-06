<template>
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<SettingsAnalyticsDashboard :is-admin="true" :user-id="userId" />
	</div>
</template>

<script setup lang="ts">
const route = useRoute();
const userId = computed(() => {
	const value = route.query.user_id;
	return Array.isArray(value) ? value[0] : value;
});
</script>
