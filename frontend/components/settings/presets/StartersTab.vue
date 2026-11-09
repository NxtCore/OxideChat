<template>
	<div class="space-y-5">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">{{ t('starters_title') }}</h3>
			<p class="text-xs text-muted-foreground">{{ t('starters_description') }}</p>
		</div>
		<div class="space-y-2">
			<ShadInput
				v-for="(_, index) in form.starters"
				:key="index"
				v-model="form.starters[index]"
				:data-testid="`preset-starter-${index}`"
				:placeholder="t('starter_placeholder')"
				maxlength="300"
			/>
		</div>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';

defineProps<{form: Record<string, any>}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`presets.editor.${key}`);
</script>
