<script setup lang="ts">
import {ref, computed, onMounted} from 'vue';
import {useThemeStore} from '~/stores/theme';
import {useMainStore} from '~/stores';
import ThemeCard from '~/components/themes/ThemeCard.vue';
import ImportThemeDialog from '~/components/themes/ImportThemeDialog.vue';
import {Button} from '~/components/ui/button';
import {Input} from '~/components/ui/input';
import {Label} from '~/components/ui/label';
import {Sun, Moon, Shuffle, Plus, RotateCcw, ExternalLink, Search, CheckCircle} from 'lucide-vue-next';

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
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between mb-6">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ mainStore.getTranslation('settings.appearance.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ mainStore.getTranslation('settings.appearance.description') }}</p>
			</div>
			<Button @click="saveThemeToPreferences">
				{{ mainStore.getTranslation('common.save') }}
			</Button>
		</div>

		<div class="space-y-8">
			<div class="space-y-4">
				<div>
					<h3 class="font-semibold text-foreground">{{ mainStore.getTranslation('settings.appearance.display_mode') }}</h3>
					<p class="mt-1 text-sm text-muted-foreground">{{ mainStore.getTranslation('settings.appearance.display_mode_description') }}</p>
				</div>

				<div class="grid max-w-xl grid-cols-2 gap-3">
					<div
						class="cursor-pointer rounded-lg border-0 bg-muted/20 p-4 transition-all duration-200 hover:bg-muted/40"
						:class="themeStore.currentMode === 'light' ? 'bg-primary/5 ring-1 ring-primary/20' : 'hover:ring-1 hover:ring-border'"
						@click="themeStore.setMode('light')"
					>
						<div class="flex items-center gap-3">
							<div class="flex h-8 w-8 items-center justify-center rounded-full bg-background">
								<Sun class="h-4 w-4 text-foreground" />
							</div>
							<div class="flex-1">
								<div class="flex items-center gap-2">
									<Label class="cursor-pointer font-medium text-foreground">{{ mainStore.getTranslation('settings.appearance.theme_light') }}</Label>
									<CheckCircle v-if="themeStore.currentMode === 'light'" class="ml-auto h-4 w-4 text-primary" />
								</div>
							</div>
						</div>
					</div>

					<div
						class="cursor-pointer rounded-lg border-0 bg-muted/20 p-4 transition-all duration-200 hover:bg-muted/40"
						:class="themeStore.currentMode === 'dark' ? 'bg-primary/5 ring-1 ring-primary/20' : 'hover:ring-1 hover:ring-border'"
						@click="themeStore.setMode('dark')"
					>
						<div class="flex items-center gap-3">
							<div class="flex h-8 w-8 items-center justify-center rounded-full bg-background">
								<Moon class="h-4 w-4 text-foreground" />
							</div>
							<div class="flex-1">
								<div class="flex items-center gap-2">
									<Label class="cursor-pointer font-medium text-foreground">{{ mainStore.getTranslation('settings.appearance.theme_dark') }}</Label>
									<CheckCircle v-if="themeStore.currentMode === 'dark'" class="ml-auto h-4 w-4 text-primary" />
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>

			<div class="space-y-4">
				<div>
					<h3 class="font-semibold text-foreground">{{ mainStore.getTranslation('settings.appearance.themes') }}</h3>
					<p class="mt-1 text-sm text-muted-foreground">{{ mainStore.getTranslation('settings.appearance.themes_description') }}</p>
				</div>

				<div class="mb-8 flex items-center gap-3">
					<div class="flex-1">
						<div class="relative">
							<Search class="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
							<Input
								v-model="searchQuery"
								:placeholder="mainStore.getTranslation('settings.appearance.search_themes')"
								class="bg-muted/20 pl-10 focus:bg-background"
							/>
						</div>
					</div>
					<Button
						variant="outline"
						size="icon"
						@click="themeStore.randomizeTheme"
						:disabled="themeStore.isLoadingThemes"
						:title="mainStore.getTranslation('settings.appearance.random_theme')"
					>
						<Shuffle class="h-4 w-4" />
					</Button>
					<Button variant="outline" size="icon" @click="themeStore.resetTheme" :title="mainStore.getTranslation('settings.appearance.reset_theme')">
						<RotateCcw class="h-4 w-4" />
					</Button>
					<Button variant="outline" @click="showImportDialog = true">
						<Plus class="h-4 w-4 mr-1" />
						{{ mainStore.getTranslation('settings.appearance.import_theme') }}
					</Button>
				</div>

				<div v-if="themeStore.isLoadingThemes" class="flex items-center justify-center gap-2 py-12 text-muted-foreground">
					<div class="h-4 w-4 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent" />
					{{ mainStore.getTranslation('settings.appearance.loading_themes') }}
				</div>

				<div v-else class="space-y-8">
					<div v-if="filteredCustomThemes.length > 0" class="space-y-3">
						<h4 class="font-medium text-sm text-muted-foreground">
							{{ mainStore.getTranslation('settings.appearance.custom_themes') }} ({{ filteredCustomThemes.length }})
						</h4>
						<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
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

					<div class="space-y-3">
						<h4 class="font-medium text-sm text-muted-foreground">
							{{ mainStore.getTranslation('settings.appearance.built_in_themes') }} ({{ filteredBuiltInThemes.length }})
						</h4>
						<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
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

					<div
						v-if="filteredBuiltInThemes.length === 0 && filteredCustomThemes.length === 0 && searchQuery"
						class="flex flex-col items-center justify-center py-12 text-center"
					>
						<Search class="mb-3 h-8 w-8 text-muted-foreground" />
						<h4 class="font-medium text-foreground">{{ mainStore.getTranslation('settings.appearance.no_themes_found') }}</h4>
						<p class="mt-1 text-sm text-muted-foreground">{{ mainStore.getTranslation('settings.appearance.try_different_search') }}</p>
					</div>
				</div>

				<div class="flex items-center justify-center gap-1 border-t border-border/50 pt-6 text-sm text-muted-foreground">
					{{ mainStore.getTranslation('settings.appearance.themes_powered_by') }}
					<a
						href="https://tweakcn.com"
						target="_blank"
						rel="noopener noreferrer"
						class="ml-1 inline-flex items-center font-medium text-primary hover:underline"
					>
						tweakcn.com
						<ExternalLink class="ml-1 h-3 w-3" />
					</a>
				</div>
			</div>
		</div>

		<ImportThemeDialog v-model:open="showImportDialog" @imported="themeStore.fetchAllThemes" />
	</div>
</template>
