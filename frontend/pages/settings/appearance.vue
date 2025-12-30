<template>
	<div class="max-w-2xl">
		<div class="rounded-lg border border-border bg-card p-6">
			<div class="mb-6">
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.appearance.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.appearance.description') }}</p>
			</div>

			<!-- Theme -->
			<div class="flex items-center justify-between py-4 border-t border-border">
				<div>
					<span class="font-medium text-foreground">{{ store.getTranslation('settings.appearance.theme') }}</span>
					<p class="text-sm text-muted-foreground">Choose between light and dark mode</p>
				</div>
				<div class="flex items-center gap-2">
					<button
						v-for="option in themeOptions"
						:key="option.value"
						@click="setTheme(option.value)"
						class="px-3 py-1.5 text-sm rounded-md transition-colors"
						:class="currentTheme === option.value ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground hover:bg-accent'"
					>
						{{ option.label }}
					</button>
				</div>
			</div>

			<!-- Language -->
			<div class="flex items-center justify-between py-4 border-t border-border">
				<div>
					<span class="font-medium text-foreground">{{ store.getTranslation('settings.appearance.language') }}</span>
					<p class="text-sm text-muted-foreground">Select your preferred language</p>
				</div>
				<select v-model="currentLanguage" class="px-3 py-1.5 text-sm rounded-md bg-muted text-foreground border border-border focus:ring-2 focus:ring-primary">
					<option value="en">English</option>
					<option value="de">Deutsch</option>
				</select>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';

const store = useMainStore();

const themeOptions = [
	{value: 'light', label: 'Light'},
	{value: 'dark', label: 'Dark'},
	{value: 'system', label: 'System'},
];

const currentTheme = ref('dark');
const currentLanguage = ref(store.base?.language || 'en');

function setTheme(theme: string) {
	currentTheme.value = theme;
}
</script>
