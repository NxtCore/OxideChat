import {fileErrorReason, fileUrl, postFile} from '~/composables/api/files';

export const IMAGE_UPLOAD_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

export const IMAGE_UPLOAD_MAX_BYTES = 2 * 1024 * 1024;

export type UploadedImage = {id: string; url: string};

export class ImageUploadError extends Error {
	constructor(readonly key: string) {
		super(key);
	}
}

export function useImageUpload() {
	const uploading = ref(false);

	async function upload(file: File): Promise<UploadedImage> {
		if (!IMAGE_UPLOAD_TYPES.includes(file.type)) throw new ImageUploadError('image_picker.errors.type');
		if (file.size > IMAGE_UPLOAD_MAX_BYTES) throw new ImageUploadError('image_picker.errors.size');
		uploading.value = true;
		try {
			const stored = await postFile(file, 'icon');
			return {id: stored.id, url: stored.url ?? fileUrl(stored.id)};
		} catch (error) {
			const reason = fileErrorReason(error);
			throw new ImageUploadError(reason ? `files.error.${reason}` : 'image_picker.errors.upload');
		} finally {
			uploading.value = false;
		}
	}

	return {uploading, upload};
}

export function firstImageFile(items: DataTransferItemList | FileList | null | undefined): File | null {
	if (!items) return null;
	for (const item of Array.from(items as ArrayLike<DataTransferItem | File>)) {
		const file = item instanceof File ? item : item.kind === 'file' ? item.getAsFile() : null;
		if (file && file.type.startsWith('image/')) return file;
	}
	return null;
}
