<template>
	<ShadDialog>
		<ShadDialogTrigger as-child class="w-full">
			<ShadButton variant="outline" class="flex-1 justify-between">
				{{ format_datetime_for_display(selected_date, selected_time) || store.getTranslation(translation_keys.select) }}
				<Calendar class="h-4 w-4" />
			</ShadButton>
		</ShadDialogTrigger>
		<ShadDialogContent class="w-fit">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation(translation_keys.select) }}</ShadDialogTitle>
			</ShadDialogHeader>
			<div class="space-y-4">
				<ShadCalendar v-model="selected_date" class="mx-auto" />
				<div v-if="enable_time" class="">
					<ShadLabel class="text-sm font-medium text-white mb-2 block">{{ store.getTranslation(translation_keys.time) }}</ShadLabel>
					<ShadInput v-model="selected_time" type="time" class="w-full" :placeholder="store.getTranslation(translation_keys.time_select)" value="00:00" />
				</div>
				<ShadDialogFooter class="flex gap-2 flex-wrap sm:justify-between">
					<ShadDialogClose as-child>
						<ShadButton @click="handle_reset" variant="outline" class="gap-0">
							<Trash2 class="h-4 w-4 mr-2" />
							{{ store.getTranslation(translation_keys.reset) }}
						</ShadButton>
					</ShadDialogClose>
					<ShadDialogClose as-child>
						<ShadButton @click="handle_submit">{{ store.getTranslation(translation_keys.ok) }}</ShadButton>
					</ShadDialogClose>
				</ShadDialogFooter>
			</div>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup>
import {Calendar, Trash2} from 'lucide-vue-next';

const store = useMainStore();

const props = defineProps({
	date: {
		type: [Date, String, null],
		default: null,
	},
	time: {
		type: String,
		default: '',
	},
	enable_time: {
		type: Boolean,
		default: true,
	},
	translation_keys: {
		type: Object,
		default: () => ({
			select: 'guild.components.dialog_calendar.date.select',
			time: 'guild.components.dialog_calendar.time.label',
			time_select: 'guild.components.dialog_calendar.time.select',
			reset: 'guild.components.dialog_calendar.reset',
			ok: 'guild.components.dialog_calendar.ok',
		}),
	},
});

const emit = defineEmits(['update:date', 'update:time', 'reset', 'submit']);

const selected_date = ref(props.date);
const selected_time = ref(props.time || '00:00');

watch(
	() => props.date,
	new_date => {
		selected_date.value = new_date;
	}
);

watch(
	() => props.time,
	new_time => {
		selected_time.value = new_time;
	}
);

const handle_reset = () => {
	selected_date.value = null;
	selected_time.value = '';
	emit('update:date', null);
	emit('update:time', '');
	emit('reset');
};

const handle_submit = () => {
	emit('update:date', selected_date.value);
	emit('update:time', selected_time.value);
	emit('submit', {
		date: selected_date.value,
		time: selected_time.value,
	});
};

const format_datetime_for_display = (date, time) => {
	if (!date) return null;

	const date_obj = new Date(date);
	const formatted_date = date_obj.toLocaleDateString();

	if (props.enable_time && time) {
		return `${formatted_date} ${time}`;
	}

	return formatted_date;
};
</script>
