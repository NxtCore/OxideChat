<template>
	<div class="flex flex-1 flex-col px-3 py-2" :class="form?.isDirty ? 'max-lg:pb-20' : ''">
		<SettingsPageHeader v-if="title" :title="title" :description="description">
			<template v-if="$slots.leading" #leading><slot name="leading" /></template>
			<template v-if="$slots.actions" #actions><slot name="actions" /></template>
		</SettingsPageHeader>

		<div class="flex-1" :class="layout === 'form' ? 'w-full max-w-3xl' : 'w-full'">
			<slot />
		</div>

		<SettingsSaveBar v-if="form" :form="form" />
	</div>
</template>

<script setup lang="ts">
import type {SettingsForm} from '~/composables/useSettingsForm';

withDefaults(defineProps<{title?: string; description?: string; layout?: 'form' | 'wide'; form?: SettingsForm}>(), {
	title: undefined,
	description: undefined,
	layout: 'wide',
	form: undefined,
});
</script>
