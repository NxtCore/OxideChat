<template>
	<Tooltip v-if="active">
		<TooltipTrigger as-child>
			<slot />
		</TooltipTrigger>
		<TooltipContent>
			<p>{{ text }}</p>
		</TooltipContent>
	</Tooltip>
	<slot v-else />
</template>

<script setup lang="ts">
import {Tooltip, TooltipContent, TooltipTrigger} from '~/components/ui/tooltip';

defineProps<{text: string; active: boolean}>();
</script>
