<template>
	<div class="flex flex-col gap-2">
		<!-- Tool call header -->
		<div class="flex cursor-pointer items-center gap-2 text-primary transition-opacity hover:opacity-80 select-none" @click="isExpanded = !isExpanded">
			<Wrench class="h-3.5 w-3.5" />
			<span class="text-[10px] font-bold uppercase tracking-widest">Tool: {{ name }}</span>
			<span v-if="isExecuting" class="flex items-center gap-1 text-xs text-muted-foreground">
				<Loader2 class="h-3 w-3 animate-spin" />
				Running...
			</span>
			<span v-else-if="error" class="flex items-center gap-1 text-xs text-destructive">
				<AlertCircle class="h-3 w-3" />
				Failed
			</span>
			<span v-else-if="output !== undefined" class="flex items-center gap-1 text-xs text-green-500">
				<CheckCircle class="h-3 w-3" />
				Complete
			</span>
			<ChevronDown class="ml-auto h-3 w-3 transition-transform" :class="isExpanded ? 'rotate-180' : ''" />
		</div>

		<!-- Expandable content -->
		<Transition name="expand">
			<div v-if="isExpanded" class="w-full rounded-xl bg-muted/50 border px-4 py-3">
				<!-- Arguments section -->
				<div class="mb-3">
					<span class="text-xs font-medium text-muted-foreground mb-1 block">Arguments</span>
					<div class="rounded-md bg-background/50 p-2 text-xs font-mono overflow-x-auto">
						<pre class="whitespace-pre-wrap">{{ formattedArgs }}</pre>
					</div>
				</div>

				<!-- Output section (if available) -->
				<div v-if="output !== undefined" class="mb-3">
					<span class="text-xs font-medium text-muted-foreground mb-1 block">Output</span>
					<div class="rounded-md bg-background/50 p-2 text-xs font-mono overflow-x-auto max-h-64 overflow-y-auto">
						<pre class="whitespace-pre-wrap">{{ formattedOutput }}</pre>
					</div>
				</div>

				<!-- Error section (if any) -->
				<div v-if="error" class="mb-3">
					<span class="text-xs font-medium text-destructive mb-1 block">Error</span>
					<div class="rounded-md bg-destructive/10 border border-destructive/20 p-2 text-xs text-destructive">
						{{ error }}
					</div>
				</div>

				<!-- Duration (if available) -->
				<div v-if="durationMs" class="text-xs text-muted-foreground">Completed in {{ durationMs }}ms</div>
			</div>
		</Transition>
	</div>
</template>

<script setup lang="ts">
import {Wrench, ChevronDown, Loader2, CheckCircle, AlertCircle} from 'lucide-vue-next';

const props = defineProps<{
	id: string;
	name: string;
	args?: Record<string, any> | string;
	output?: any;
	error?: string;
	isExecuting?: boolean;
	durationMs?: number;
}>();

const isExpanded = ref(true);

const formattedArgs = computed(() => {
	if (typeof props.args === 'string') {
		try {
			return JSON.stringify(JSON.parse(props.args), null, 2);
		} catch {
			return props.args;
		}
	}
	return JSON.stringify(props.args || {}, null, 2);
});

const formattedOutput = computed(() => {
	if (typeof props.output === 'string') {
		return props.output;
	}
	return JSON.stringify(props.output, null, 2);
});
</script>

<style scoped>
.expand-enter-active,
.expand-leave-active {
	transition: all 0.3s ease;
	overflow: hidden;
}

.expand-enter-from,
.expand-leave-to {
	opacity: 0;
	max-height: 0;
	padding-top: 0;
	padding-bottom: 0;
}

.expand-enter-to,
.expand-leave-from {
	max-height: 500px;
}
</style>
