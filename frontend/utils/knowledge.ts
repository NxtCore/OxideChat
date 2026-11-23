import type {ExternalView} from '~/types/generated/ExternalView';

export function emptyView(): ExternalView {
	return {
		connection_id: '',
		collection: '',
		vector_name: null,
		distance: 'cosine',
		embedding: {model_id: '', dimensions: null},
		fields: {text: '', title: null, url: null, section: null},
		filter: [],
		access: null,
	};
}

export function parseFilterValue(input: string): string | number | boolean {
	const text = input.trim();
	if (text === 'true' || text === 'false') return text === 'true';
	if (/^-?\d+(\.\d+)?$/.test(text)) return Number(text);
	return input;
}
