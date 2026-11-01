import {onBeforeRouteLeave} from 'vue-router';
import {useMainStore} from '~/stores';

export interface SettingsFormOptions<T> {
	value: () => T;
	restore: (saved: T) => void;
	save: () => Promise<void>;
	savedKey: string;
	errorKey: string;
	normalize?: (value: T) => unknown;
	canEdit?: () => boolean;
}

export function useSettingsForm<T>(options: SettingsFormOptions<T>) {
	const store = useMainStore();

	const comparable = (value: T) => JSON.stringify(options.normalize ? options.normalize(value) : value);

	const savedValue = ref(JSON.stringify(options.value()));
	const savedComparable = ref(comparable(options.value()));
	const saving = ref(false);
	const leavePromptOpen = ref(false);
	let answerLeavePrompt: ((discard: boolean) => void) | null = null;

	const isDirty = computed(() => (options.canEdit?.() ?? true) && comparable(options.value()) !== savedComparable.value);

	function markSaved() {
		savedValue.value = JSON.stringify(options.value());
		savedComparable.value = comparable(options.value());
	}

	function reset() {
		options.restore(JSON.parse(savedValue.value) as T);
	}

	async function save(): Promise<boolean> {
		if (saving.value || !isDirty.value) return false;
		saving.value = true;
		try {
			await options.save();
			markSaved();
			store.toast(store.getTranslation(options.savedKey), {type: 'success'});
			return true;
		} catch (e: any) {
			store.toast(store.getTranslation(options.errorKey), {type: 'error', description: errorMessage(e, '')});
			return false;
		} finally {
			saving.value = false;
		}
	}

	function confirmDiscard(): Promise<boolean> {
		if (!isDirty.value) return Promise.resolve(true);
		leavePromptOpen.value = true;
		return new Promise(resolve => {
			answerLeavePrompt = resolve;
		});
	}

	function answerLeave(discard: boolean) {
		leavePromptOpen.value = false;
		if (discard) reset();
		answerLeavePrompt?.(discard);
		answerLeavePrompt = null;
	}

	function warnBeforeUnload(event: BeforeUnloadEvent) {
		if (isDirty.value) event.preventDefault();
	}

	onBeforeRouteLeave(() => confirmDiscard());
	onMounted(() => window.addEventListener('beforeunload', warnBeforeUnload));
	onBeforeUnmount(() => window.removeEventListener('beforeunload', warnBeforeUnload));

	return reactive({isDirty, saving, leavePromptOpen, save, reset, markSaved, confirmDiscard, answerLeave});
}

export type SettingsForm = ReturnType<typeof useSettingsForm>;
