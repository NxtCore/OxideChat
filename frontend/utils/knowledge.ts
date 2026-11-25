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

/** The typed value in the type `example` has (a sample record's value of the field); without one, only text that reads back unchanged becomes a number, so `007` stays text. */
export function parseFilterValue(input: string, example?: unknown): string | number | boolean {
	const text = input.trim();
	if (typeof example === 'string') return input;
	if (text === 'true' || text === 'false') return text === 'true';
	if (typeof example === 'number' && text !== '' && Number.isFinite(Number(text))) return Number(text);
	if (example === undefined && /^-?\d+(\.\d+)?$/.test(text) && String(Number(text)) === text) return Number(text);
	return input;
}

/** The first value at a dotted `path` among sample records. */
export function sampleValue(sample: unknown[], path: string): unknown {
	return sample
		.map(record => path.split('.').reduce<any>((value, key) => (value && typeof value === 'object' ? value[key] : undefined), record))
		.find(value => value !== undefined && value !== null);
}
