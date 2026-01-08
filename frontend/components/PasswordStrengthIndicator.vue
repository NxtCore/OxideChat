<template>
	<div class="password-strength-indicator flex flex-col gap-2">
		<div class="flex items-center gap-2">
			<div class="flex-1 h-2 bg-muted rounded-full overflow-hidden">
				<div class="h-full transition-all duration-300 rounded-full" :class="strengthBarClass" :style="{width: `${strengthPercentage}%`}" />
			</div>
		</div>

		<div class="grid grid-cols-2 gap-1 text-xs" v-if="showRequirements">
			<div
				v-for="req in requirements"
				:key="req.key"
				class="flex items-center gap-1.5 transition-colors duration-200"
				:class="req.met ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'"
			>
				<component :is="req.met ? CheckCircle2 : Circle" class="size-3.5 shrink-0" :class="req.met ? '' : 'opacity-50'" />
				<span>{{ req.label }}</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {computed} from 'vue';
import {CheckCircle2, Circle} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();

interface Props {
	password: string;
	showRequirements?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
	showRequirements: true,
});

// Password requirements matching server-side validation
const MIN_LENGTH = 8;
const MAX_LENGTH = 128;

const requirements = computed(() => [
	{
		key: 'length',
		label: store.getTranslation('auth.password.min_length', {min: MIN_LENGTH.toString()}) || `At least ${MIN_LENGTH} characters`,
		met: props.password.length >= MIN_LENGTH,
	},
	{
		key: 'maxLength',
		label: store.getTranslation('auth.password.max_length', {max: MAX_LENGTH.toString()}) || `Maximum ${MAX_LENGTH} characters`,
		met: props.password.length > 0 && props.password.length <= MAX_LENGTH,
	},
	{
		key: 'uppercase',
		label: store.getTranslation('auth.password.uppercase') || 'One uppercase letter',
		met: /[A-Z]/.test(props.password),
	},
	{
		key: 'lowercase',
		label: store.getTranslation('auth.password.lowercase') || 'One lowercase letter',
		met: /[a-z]/.test(props.password),
	},
	{
		key: 'digit',
		label: store.getTranslation('auth.password.digit') || 'One number',
		met: /\d/.test(props.password),
	},
	{
		key: 'special',
		label: store.getTranslation('auth.password.special') || 'One special character',
		met: /[^a-zA-Z0-9]/.test(props.password),
	},
]);

// Filter out max length for display (it's always met unless user types >128 chars)
const displayRequirements = computed(() => requirements.value.filter(r => r.key !== 'maxLength' || props.password.length > MAX_LENGTH));

const metCount = computed(() => requirements.value.filter(r => r.met).length);
const totalRequirements = computed(() => requirements.value.length);

const strengthPercentage = computed(() => {
	if (props.password.length === 0) return 0;
	return (metCount.value / totalRequirements.value) * 100;
});

const strengthLevel = computed(() => {
	const percentage = strengthPercentage.value;
	if (percentage === 0) return 'none';
	if (percentage <= 33) return 'weak';
	if (percentage <= 66) return 'fair';
	if (percentage < 100) return 'good';
	return 'strong';
});

const strengthBarClass = computed(() => {
	switch (strengthLevel.value) {
		case 'weak':
			return 'bg-red-500';
		case 'fair':
			return 'bg-orange-500';
		case 'good':
			return 'bg-yellow-500';
		case 'strong':
			return 'bg-green-500';
		default:
			return 'bg-muted';
	}
});

const strengthTextClass = computed(() => {
	switch (strengthLevel.value) {
		case 'weak':
			return 'text-red-500';
		case 'fair':
			return 'text-orange-500';
		case 'good':
			return 'text-yellow-600 dark:text-yellow-500';
		case 'strong':
			return 'text-green-500';
		default:
			return 'text-muted-foreground';
	}
});
// Expose validation status for parent components
const isValid = computed(() => metCount.value === totalRequirements.value);

defineExpose({
	isValid,
	metCount,
	totalRequirements,
});
</script>
