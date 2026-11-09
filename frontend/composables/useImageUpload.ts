export const IMAGE_UPLOAD_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

export const IMAGE_UPLOAD_MAX_BYTES = 2 * 1024 * 1024;

export type UploadedImage = {id: string; url: string};

export class ImageUploadError extends Error {
	constructor(readonly key: string) {
		super(key);
	}
}

function readAsDataUrl(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => resolve(reader.result as string);
		reader.onerror = () => reject(reader.error);
		reader.readAsDataURL(file);
	});
}

export function useImageUpload() {
	const uploading = ref(false);

	async function upload(file: File): Promise<UploadedImage> {
		if (!IMAGE_UPLOAD_TYPES.includes(file.type)) throw new ImageUploadError('image_picker.errors.type');
		if (file.size > IMAGE_UPLOAD_MAX_BYTES) throw new ImageUploadError('image_picker.errors.size');
		uploading.value = true;
		try {
			const response = await postImage(await readAsDataUrl(file));
			if (!response.ok) throw new ImageUploadError('image_picker.errors.upload');
			return (await response.json()) as UploadedImage;
		} catch (error) {
			throw error instanceof ImageUploadError ? error : new ImageUploadError('image_picker.errors.upload');
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
