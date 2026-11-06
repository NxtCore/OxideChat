<template>
	<div class="mb-6 px-3 lg:mb-8 lg:shrink-0" data-testid="area-header">
		<button
			class="group -mx-3 mb-4 flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
			data-testid="area-header-back"
			@click="$emit('back')"
		>
			<ArrowLeft class="h-4 w-4 transition-transform group-hover:-translate-x-1" />
			<span>{{ backLabel }}</span>
		</button>
		<h1 class="text-3xl font-bold tracking-tight text-foreground" data-testid="area-header-title">{{ title }}</h1>
		<p class="text-base text-muted-foreground">{{ description }}</p>
	</div>
</template>

<script setup lang="ts">
import {ArrowLeft} from 'lucide-vue-next';

defineProps<{title: string; description: string; backLabel: string}>();
defineEmits<{back: []}>();
</script>
