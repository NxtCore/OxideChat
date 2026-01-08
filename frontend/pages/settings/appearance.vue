<template>
	<div class="max-w-2xl">
		<div class="rounded-lg border border-border bg-card p-6">
			<div class="mb-6">
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.appearance.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.appearance.description') }}</p>
			</div>

			<div class="flex items-center justify-between py-4 border-t border-border">
				<div>
					<span class="font-medium text-foreground">{{ store.getTranslation('settings.appearance.theme') }}</span>
					<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.appearance.theme_description') }}</p>
				</div>
				<div class="flex items-center gap-2">
					<button
						v-for="option in themeOptions"
						:key="option.value"
						@click="setTheme(option.value)"
						class="px-3 py-1.5 text-sm rounded-md transition-colors"
						:class="currentTheme === option.value ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground hover:bg-accent'"
					>
						{{ store.getTranslation(option.translationKey) }}
					</button>
				</div>
			</div>

			<div class="flex items-center justify-between py-4 border-t border-border">
				<div>
					<span class="font-medium text-foreground">{{ store.getTranslation('settings.appearance.language') }}</span>
					<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.appearance.language_description') }}</p>
				</div>
				<select v-model="currentLanguage" class="px-3 py-1.5 text-sm rounded-md bg-muted text-foreground border border-border focus:ring-2 focus:ring-primary">
					<option value="en">{{ store.getTranslation('settings.appearance.language_en') }}</option>
					<option value="de">{{ store.getTranslation('settings.appearance.language_de') }}</option>
				</select>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';

const store = useMainStore();

const themeOptions = [
	{value: 'light', translationKey: 'settings.appearance.theme_light'},
	{value: 'dark', translationKey: 'settings.appearance.theme_dark'},
	{value: 'system', translationKey: 'settings.appearance.theme_system'},
];

const currentTheme = ref('dark');
const currentLanguage = ref(store.base?.language || 'en');

function setTheme(theme: string) {
	currentTheme.value = theme;
}
</script>
