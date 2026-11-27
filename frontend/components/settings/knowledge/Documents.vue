<template>
	<div class="space-y-4" data-testid="knowledge-documents">
		<div>
			<h3 class="text-base font-semibold text-foreground">{{ t('title') }}</h3>
			<p class="text-sm text-muted-foreground">{{ t('description') }}</p>
		</div>

		<div
			v-if="canEdit"
			class="flex cursor-pointer flex-col items-center justify-center rounded-xl border border-dashed border-border bg-card px-6 py-8 text-center transition-colors hover:bg-accent/30"
			:class="dragging ? 'border-primary bg-primary/5' : ''"
			data-testid="knowledge-documents-dropzone"
			@click="fileInput?.click()"
			@dragover.prevent="dragging = true"
			@dragleave.prevent="dragging = false"
			@drop.prevent="onDrop"
		>
			<input ref="fileInput" type="file" multiple class="hidden" data-testid="knowledge-documents-input" @change="onSelect" />
			<Loader2 v-if="uploading" class="mb-2 h-6 w-6 animate-spin text-primary" />
			<Upload v-else class="mb-2 h-6 w-6 text-primary" />
			<p class="text-sm font-medium text-foreground">{{ uploading ? t('uploading') : t('drop_title') }}</p>
			<p class="mt-1 text-xs text-muted-foreground">{{ t('drop_hint') }}</p>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-8 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>

		<p v-else-if="documents.length === 0" class="py-6 text-center text-sm text-muted-foreground" data-testid="knowledge-documents-empty">{{ t('empty') }}</p>

		<div v-else class="space-y-2">
			<div
				v-for="doc in documents"
				:key="doc.id"
				class="flex flex-wrap items-center gap-3 rounded-lg border border-border bg-card px-4 py-3"
				:data-testid="`knowledge-document-${doc.filename}`"
			>
				<FileText class="h-4 w-4 shrink-0 text-muted-foreground" />
				<div class="min-w-0 flex-1">
					<div class="flex flex-wrap items-center gap-2">
						<span class="truncate text-sm font-medium text-foreground">{{ doc.filename }}</span>
						<span
							class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
							:class="statusClass(doc.status)"
							:data-testid="`knowledge-document-status-${doc.filename}`"
						>
							<Loader2 v-if="doc.status === 'processing' || doc.status === 'deleting'" class="h-3 w-3 animate-spin" />
							{{ t(`status.${doc.status}`) }}
						</span>
						<span
							v-if="doc.access_restricted"
							class="inline-flex items-center gap-1 rounded-full border border-border bg-muted px-2 py-0.5 text-xs text-muted-foreground"
							:data-testid="`knowledge-document-restricted-${doc.filename}`"
						>
							<Lock class="h-3 w-3" />
							{{ t('restricted') }}
						</span>
					</div>
					<p class="mt-0.5 text-xs text-muted-foreground">
						{{ metaSummary(doc) }}
					</p>
					<p v-if="doc.status === 'failed' && doc.error" class="mt-1 text-xs text-destructive" :data-testid="`knowledge-document-error-${doc.filename}`">
						{{ ragDocErrorMessage(doc.error) }}
					</p>
				</div>

				<div v-if="canEdit" class="flex items-center gap-2">
					<ShadButton
						v-if="doc.status === 'failed' && doc.can_retry"
						variant="outline"
						size="sm"
						:data-testid="`knowledge-document-retry-${doc.filename}`"
						@click="retry(doc)"
					>
						<RotateCw class="h-3.5 w-3.5" />
						{{ t('retry') }}
					</ShadButton>
					<ShadButton
						v-if="doc.status !== 'deleting'"
						variant="outline"
						size="sm"
						:data-testid="`knowledge-document-access-${doc.filename}`"
						@click="openAccess(doc)"
					>
						<Lock class="h-3.5 w-3.5" />
						{{ t('limit_access') }}
					</ShadButton>
					<ShadButton
						v-if="doc.status !== 'deleting'"
						variant="ghost"
						size="icon"
						:aria-label="store.getTranslation('common.delete')"
						:data-testid="`knowledge-document-delete-${doc.filename}`"
						@click="remove(doc)"
					>
						<Trash2 class="h-4 w-4 text-muted-foreground hover:text-destructive" />
					</ShadButton>
				</div>
			</div>
		</div>

		<ShadDialog :open="accessDoc !== null" @update:open="open => !open && (accessDoc = null)">
			<ShadDialogContent class="sm:max-w-[540px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ accessDoc ? t('access_title', {name: accessDoc.filename}) : '' }}</ShadDialogTitle>
					<ShadDialogDescription>{{ t('access_description') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<SettingsAssignmentPicker
					v-if="accessDoc"
					:model-value="accessData?.targets ?? []"
					:options="accessOptions"
					:kinds="accessData?.kinds ?? []"
					:roles="[]"
					:disabled="!accessData?.kinds.length"
					:describe="describeAccess"
					:empty-hint="t('access_empty')"
					test-id="knowledge-document-access-picker"
					@update:model-value="saveAccess"
					@search="searchUsers"
				/>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {FileText, Loader2, Lock, RotateCw, Trash2, Upload} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AssignmentOption, AssignmentTarget} from '~/types/assignments';
import type {DocumentStatus} from '~/types/generated/DocumentStatus';
import type {RagDocumentResponse} from '~/types/generated/RagDocumentResponse';

const props = defineProps<{id: string; canEdit: boolean}>();
const store = useMainStore();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`knowledge.documents.${key}`, params);

const fileInput = ref<HTMLInputElement | null>(null);
const loading = ref(true);
const uploading = ref(false);
const dragging = ref(false);
const documents = ref<RagDocumentResponse[]>([]);
const accessDoc = ref<RagDocumentResponse | null>(null);

const {data: accessData, load: loadAccess, replace: replaceAccess} = useAssignments();
const {teams, roles, users, searchUsers, load: loadOptions} = useAssignmentOptions();
const accessOptions = computed<AssignmentOption[]>(() => [...teams.value, ...roles.value, ...users.value]);

let pollTimer: ReturnType<typeof setInterval> | null = null;

function needsPolling() {
	return documents.value.some(doc => doc.status === 'pending' || doc.status === 'processing' || doc.status === 'deleting');
}

function syncPolling() {
	if (needsPolling() && !pollTimer) {
		pollTimer = setInterval(() => void refresh(true), 2000);
	} else if (!needsPolling() && pollTimer) {
		clearInterval(pollTimer);
		pollTimer = null;
	}
}

async function refresh(silent = false) {
	try {
		documents.value = await knowledgeDocuments(props.id);
		syncPolling();
	} catch (error: any) {
		if (!silent) store.toast(t('load_error'), {type: 'error', description: ragErrorMessage(error, '')});
	} finally {
		loading.value = false;
	}
}

function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function metaSummary(doc: RagDocumentResponse): string {
	const parts = [formatBytes(doc.size_bytes)];
	if (doc.page_count) parts.push(t('pages', {count: String(doc.page_count)}));
	if (doc.chunk_count > 0) parts.push(t('chunks', {count: String(doc.chunk_count)}));
	return parts.join(' · ');
}

function statusClass(status: DocumentStatus): string {
	switch (status) {
		case 'ready':
			return 'bg-primary/10 text-primary';
		case 'failed':
			return 'bg-destructive/10 text-destructive';
		default:
			return 'bg-muted text-muted-foreground';
	}
}

async function uploadFiles(files: FileList | File[]) {
	const list = Array.from(files);
	if (!list.length) return;
	uploading.value = true;
	try {
		for (const file of list) {
			try {
				await uploadKnowledgeDocument(props.id, file);
				store.toast(t('upload_success'), {type: 'success'});
			} catch (error: any) {
				store.toast(t('upload_error'), {type: 'error', description: ragErrorMessage(error, '')});
			}
		}
		await refresh(true);
	} finally {
		uploading.value = false;
		if (fileInput.value) fileInput.value.value = '';
	}
}

function onSelect(event: Event) {
	const input = event.target as HTMLInputElement;
	if (input.files) void uploadFiles(input.files);
}

function onDrop(event: DragEvent) {
	dragging.value = false;
	if (event.dataTransfer?.files) void uploadFiles(event.dataTransfer.files);
}

async function retry(doc: RagDocumentResponse) {
	try {
		await retryKnowledgeDocument(props.id, doc.id);
		store.toast(t('retry_success'), {type: 'success'});
		await refresh(true);
	} catch (error: any) {
		store.toast(t('upload_error'), {type: 'error', description: ragErrorMessage(error, '')});
	}
}

async function remove(doc: RagDocumentResponse) {
	try {
		await deleteKnowledgeDocument(props.id, doc.id);
		store.toast(t('delete_success'), {type: 'success'});
		await refresh(true);
	} catch (error: any) {
		store.toast(t('delete_error'), {type: 'error', description: ragErrorMessage(error, '')});
	}
}

async function openAccess(doc: RagDocumentResponse) {
	accessDoc.value = doc;
	await Promise.all([loadAccess(documentAssignmentsPath(props.id, doc.id)), loadOptions()]);
}

function describeAccess(target: AssignmentOption) {
	return t(`access_effect.${target.kind}`, {name: target.label});
}

async function saveAccess(targets: AssignmentTarget[]) {
	if (!accessDoc.value) return;
	await replaceAccess(documentAssignmentsPath(props.id, accessDoc.value.id), targets);
	await refresh(true);
}

onMounted(() => void refresh());

onBeforeUnmount(() => {
	if (pollTimer) clearInterval(pollTimer);
});
</script>
