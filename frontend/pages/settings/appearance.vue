<script setup lang="ts">
import {ref, computed, onMounted} from 'vue';
import {useThemeStore} from '~/stores/theme';
import {useMainStore} from '~/stores';
import ThemeCard from '~/components/themes/ThemeCard.vue';
import ImportThemeDialog from '~/components/themes/ImportThemeDialog.vue';
import {Button} from '~/components/ui/button';
import {Input} from '~/components/ui/input';
import {Label} from '~/components/ui/label';
import {Separator} from '~/components/ui/separator';
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from '~/components/ui/card';
import {Skeleton} from '~/components/ui/skeleton';
import {Sun, Moon, Monitor, Shuffle, Plus, RotateCcw, ExternalLink} from 'lucide-vue-next';

const themeStore = useThemeStore();
const mainStore = useMainStore();

const searchQuery = ref('');
const showImportDialog = ref(false);

const filteredBuiltInThemes = computed(() => {
	if (!searchQuery.value) return themeStore.builtInThemes;
	return themeStore.builtInThemes.filter(t => t.name.toLowerCase().includes(searchQuery.value.toLowerCase()));
});

const filteredCustomThemes = computed(() => {
	if (!searchQuery.value) return themeStore.customThemes;
	return themeStore.customThemes.filter(t => t.name.toLowerCase().includes(searchQuery.value.toLowerCase()));
});

function handleThemeDelete(url: string) {
	themeStore.removeCustomTheme(url);
	mainStore.toast(mainStore.getTranslation('settings.theme_deleted'), {type: 'success'});
}

async function saveThemeToPreferences() {
	try {
		const {$customFetch} = useNuxtApp();
		await $customFetch('/api/v1/users/@me/preferences', {
			method: 'PATCH',
			body: {
				theme_css_vars: themeStore.cssVars,
				custom_theme_urls: themeStore.customThemeUrls,
			},
		});
		mainStore.toast(mainStore.getTranslation('settings.theme_saved'), {type: 'success'});
	} catch (e) {
		mainStore.toast(mainStore.getTranslation('common.error'), {type: 'error'});
	}
}

onMounted(() => {
	themeStore.fetchAllThemes();
});
</script>

<template>
	<div class="space-y-6">
		<Card>
			<CardHeader>
				<CardTitle>{{ mainStore.getTranslation('settings.appearance') }}</CardTitle>
				<CardDescription>{{ mainStore.getTranslation('settings.appearance_description') }}</CardDescription>
			</CardHeader>
			<CardContent class="space-y-6">
				<div class="space-y-3">
					<Label>{{ mainStore.getTranslation('settings.color_mode') }}</Label>
					<div class="flex gap-2">
						<Button
							variant="outline"
							size="sm"
							:class="{'border-primary bg-primary/10': themeStore.currentMode === 'light'}"
							@click="themeStore.setMode('light')"
						>
							<Sun class="w-4 h-4 mr-2" />
							{{ mainStore.getTranslation('settings.light') }}
						</Button>
						<Button
							variant="outline"
							size="sm"
							:class="{'border-primary bg-primary/10': themeStore.currentMode === 'dark'}"
							@click="themeStore.setMode('dark')"
						>
							<Moon class="w-4 h-4 mr-2" />
							{{ mainStore.getTranslation('settings.dark') }}
						</Button>
					</div>
				</div>

				<Separator />

				<div class="space-y-3">
					<div class="flex items-center justify-between">
						<Label>{{ mainStore.getTranslation('settings.theme') }}</Label>
						<div class="flex gap-2">
							<Button variant="outline" size="sm" @click="themeStore.randomizeTheme">
								<Shuffle class="w-4 h-4 mr-1" />
								{{ mainStore.getTranslation('settings.randomize') }}
							</Button>
							<Button variant="outline" size="sm" @click="themeStore.resetTheme">
								<RotateCcw class="w-4 h-4 mr-1" />
								{{ mainStore.getTranslation('settings.reset') }}
							</Button>
							<Button variant="outline" size="sm" @click="showImportDialog = true">
								<Plus class="w-4 h-4 mr-1" />
								{{ mainStore.getTranslation('settings.import') }}
							</Button>
						</div>
					</div>

					<Input v-model="searchQuery" :placeholder="mainStore.getTranslation('settings.search_themes')" class="max-w-sm" />
				</div>

				<div v-if="themeStore.isLoadingThemes" class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
					<Skeleton v-for="i in 8" :key="i" class="h-24 rounded-lg" />
				</div>

				<div v-else class="space-y-4">
					<div v-if="filteredCustomThemes.length > 0" class="space-y-2">
						<Label class="text-xs text-muted-foreground uppercase">
							{{ mainStore.getTranslation('settings.custom_themes') }}
						</Label>
						<div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
							<ThemeCard
								v-for="theme in filteredCustomThemes"
								:key="theme.url"
								:theme="theme"
								:is-selected="themeStore.selectedThemeUrl === theme.url"
								:mode="themeStore.currentMode"
								:can-delete="true"
								@select="themeStore.selectTheme(theme)"
								@delete="handleThemeDelete(theme.url)"
							/>
						</div>
					</div>

					<div class="space-y-2">
						<Label class="text-xs text-muted-foreground uppercase">
							{{ mainStore.getTranslation('settings.built_in_themes') }}
						</Label>
						<div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
							<ThemeCard
								v-for="theme in filteredBuiltInThemes"
								:key="theme.url"
								:theme="theme"
								:is-selected="themeStore.selectedThemeUrl === theme.url"
								:mode="themeStore.currentMode"
								@select="themeStore.selectTheme(theme)"
							/>
						</div>
					</div>
				</div>

				<Separator />

				<div class="flex items-center justify-between">
					<div class="text-sm text-muted-foreground">
						{{ mainStore.getTranslation('settings.themes_powered_by') }}
						<a href="https://tweakcn.com" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1 text-primary hover:underline">
							tweakcn.com
							<ExternalLink class="w-3 h-3" />
						</a>
					</div>
					<Button @click="saveThemeToPreferences">
						{{ mainStore.getTranslation('common.save') }}
					</Button>
				</div>
			</CardContent>
		</Card>

		<ImportThemeDialog v-model:open="showImportDialog" @imported="themeStore.fetchAllThemes" />
	</div>
</template>
