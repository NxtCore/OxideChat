<template>
	<ShadPopover v-if="preset && current" v-model:open="open">
		<ShadPopoverTrigger as-child>
			<button
				type="button"
				data-testid="composer-level"
				:data-level="current.key"
				:aria-label="`${t('chat.level.title')}: ${levelLabel(current)}`"
				:disabled="chatStore.isStreaming"
				class="inline-flex h-8 shrink-0 items-center gap-1 rounded-full bg-muted/60 px-3 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
			>
				<span class="max-w-28 truncate">{{ levelLabel(current) }}</span>
				<ChevronDown class="h-3 w-3 text-muted-foreground" />
			</button>
		</ShadPopoverTrigger>
		<ShadPopoverContent align="end" class="w-64 max-w-[calc(100vw-1rem)] rounded-2xl p-3" data-testid="composer-level-panel">
			<div class="relative px-6 text-center">
				<p class="truncate text-sm font-semibold text-foreground" data-testid="composer-level-title">{{ levelLabel(current) }}</p>
				<p class="truncate text-xs text-muted-foreground">{{ current.model?.display_name ?? current.model_id }}</p>
				<button
					v-if="defaultKey && current.key !== defaultKey"
					type="button"
					data-testid="composer-level-reset"
					:aria-label="t('chat.level.reset')"
					:title="t('chat.level.reset')"
					class="absolute right-0 top-0 rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
					@click="select(defaultKey)"
				>
					<RotateCcw class="h-3.5 w-3.5" />
				</button>
			</div>

			<div
				ref="track"
				role="radiogroup"
				:aria-label="t('chat.level.title')"
				class="relative mt-3 h-8 cursor-pointer touch-none select-none rounded-full bg-muted"
				@pointerdown="onPointerDown"
				@pointermove="onPointerMove"
				@pointerup="dragging = false"
				@pointercancel="dragging = false"
				@keydown="onKeydown"
			>
				<div class="absolute inset-y-0 left-0 rounded-full bg-foreground transition-[width] duration-150" :style="{width: `calc(${position(currentIndex)} + ${THUMB / 2}px)`}" />
				<span
					v-for="(level, index) in levels"
					:key="`dot-${level.key}`"
					class="pointer-events-none absolute top-1/2 h-1 w-1 -translate-x-1/2 -translate-y-1/2 rounded-full"
					:class="[index <= currentIndex ? 'bg-background/60' : 'bg-muted-foreground/60', isUsable(level) ? '' : 'opacity-30']"
					:style="{left: position(index)}"
				/>
				<button
					v-for="(level, index) in levels"
					:key="level.key"
					type="button"
					role="radio"
					data-testid="composer-level-option"
					:data-level="level.key"
					:aria-label="levelLabel(level)"
					:aria-checked="level.key === current.key"
					:aria-disabled="!isUsable(level)"
					:tabindex="level.key === current.key ? 0 : -1"
					:title="problem(level) ?? levelLabel(level)"
					class="absolute top-0 h-8 w-8 -translate-x-1/2 rounded-full transition-[left] duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
					:class="level.key === current.key ? 'border border-border bg-background shadow-md' : ''"
					:style="{left: position(index)}"
					@pointerdown.stop="onPointerDown"
					@click="select(level.key)"
				/>
			</div>

			<p v-if="current.hint" class="mt-2.5 text-center text-xs text-muted-foreground" data-testid="composer-level-hint">{{ current.hint }}</p>
		</ShadPopoverContent>
	</ShadPopover>
</template>

<script setup lang="ts">
import {ChevronDown, RotateCcw} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import type {PresetLevelResponse} from '~/types/presets';

const THUMB = 32;

const store = useMainStore();
const chatStore = useChatStore();
const modelStore = useModelStore();
const open = ref(false);
const track = ref<HTMLElement | null>(null);
const dragging = ref(false);

const t = (key: string) => store.getTranslation(key);

const preset = computed(() => (hasLevels(modelStore.selectedModel?.preset) ? modelStore.selectedModel!.preset! : null));
const levels = computed(() => preset.value?.levels ?? []);
const current = computed(() => modelStore.selectedModel?.level ?? null);
const currentIndex = computed(() => Math.max(levels.value.findIndex(level => level.key === current.value?.key), 0));
const defaultKey = computed(() => {
	const key = preset.value?.default_level;
	const level = levels.value.find(candidate => candidate.key === key);
	return level && isUsable(level) ? level.key : null;
});

function position(index: number): string {
	const steps = Math.max(levels.value.length - 1, 1);
	return `calc(${THUMB / 2}px + (100% - ${THUMB}px) * ${index / steps})`;
}

function levelLabel(level: PresetLevelResponse) {
	return presetLevelLabel(level, store.getTranslation);
}

function problem(level: PresetLevelResponse): string | null {
	if (level.unavailable) return t(`presets.unavailable.${level.unavailable}`);
	if (level.model?.budget_blocked) return t('settings.budgets.reached');
	return null;
}

function isUsable(level: PresetLevelResponse) {
	return isLevelUsable(level);
}

function select(key: string) {
	const level = levels.value.find(candidate => candidate.key === key);
	if (level && isUsable(level)) modelStore.setPresetLevel(key);
}

function nearestUsable(clientX: number): PresetLevelResponse | null {
	const rect = track.value?.getBoundingClientRect();
	if (!rect || levels.value.length < 2) return null;
	const ratio = Math.min(Math.max((clientX - rect.left - THUMB / 2) / (rect.width - THUMB), 0), 1);
	const target = ratio * (levels.value.length - 1);
	return (
		levels.value
			.map((level, index) => ({level, distance: Math.abs(index - target)}))
			.filter(({level}) => isUsable(level))
			.sort((a, b) => a.distance - b.distance)[0]?.level ?? null
	);
}

function onPointerDown(event: PointerEvent) {
	dragging.value = true;
	track.value?.setPointerCapture?.(event.pointerId);
	const level = nearestUsable(event.clientX);
	if (level) select(level.key);
}

function onPointerMove(event: PointerEvent) {
	if (!dragging.value) return;
	const level = nearestUsable(event.clientX);
	if (level) select(level.key);
}

function onKeydown(event: KeyboardEvent) {
	const step = event.key === 'ArrowRight' || event.key === 'ArrowUp' ? 1 : event.key === 'ArrowLeft' || event.key === 'ArrowDown' ? -1 : 0;
	if (!step) return;
	event.preventDefault();
	for (let index = currentIndex.value + step; index >= 0 && index < levels.value.length; index += step) {
		const level = levels.value[index];
		if (level && isUsable(level)) {
			select(level.key);
			nextTick(() => track.value?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus());
			return;
		}
	}
}
</script>
