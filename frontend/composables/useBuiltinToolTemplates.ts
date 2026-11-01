import {Globe, Sparkles} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

export function useBuiltinToolTemplates() {
	const store = useMainStore();

	return computed(() => [
		{
			name: 'websearch',
			display_name: 'Web Search',
			description: 'Search web using Exa or Tavily for current and accurate information',
			source_kind: 'BUILTIN',
			icon: Globe,
			source_config: {builtin_id: 'websearch'},
			functions: [
				{
					name: 'websearch',
					description: 'Search web for current information',
					input_schema: {
						type: 'object',
						properties: {
							query: {type: 'string', description: 'Search query'},
							num_results: {type: 'integer', description: 'Number of results (1-20)', default: 10},
						},
						required: ['query'],
					},
				},
				{
					name: 'crawl',
					description: 'Crawl a single website content',
					input_schema: {
						type: 'object',
						properties: {
							url: {type: 'string', description: 'URL to crawl'},
						},
						required: ['url'],
					},
				},
			],
			settings_schema: {
				type: 'object',
				required: ['api_key', 'provider'],
				properties: {
					api_key: {type: 'string', title: 'API Key', secret: true, description: 'Get your API key from specified provider'},
					provider: {
						type: 'string',
						title: 'Provider',
						enum: ['exa', 'tavily'],
						description: 'Web search provider to use',
					},
				},
			},
		},
		{
			name: 'imagegen',
			display_name: store.getTranslation('settings.tools.imagegen.display_name'),
			description: store.getTranslation('settings.tools.imagegen.description'),
			source_kind: 'BUILTIN',
			icon: Sparkles,
			source_config: {builtin_id: 'imagegen'},
			functions: [
				{
					name: 'generate',
					description: store.getTranslation('settings.tools.imagegen.generate.description'),
					input_schema: {
						type: 'object',
						properties: {
							prompt: {type: 'string', description: store.getTranslation('settings.tools.imagegen.generate.prompt')},
							size: {
								type: 'string',
								description: store.getTranslation('settings.tools.imagegen.generate.size'),
								enum: ['1024x1024', '1792x1024', '1024x1792', '512x512', '256x256'],
								default: '1024x1024',
							},
							quality: {
								type: 'string',
								description: store.getTranslation('settings.tools.imagegen.generate.quality'),
								enum: ['auto', 'low', 'medium', 'high'],
								default: 'auto',
							},
						},
						required: ['prompt'],
					},
				},
				{
					name: 'edit',
					description: store.getTranslation('settings.tools.imagegen.edit.description'),
					input_schema: {
						type: 'object',
						properties: {
							image_id: {type: 'string', description: store.getTranslation('settings.tools.imagegen.edit.image_id')},
							image_url: {type: 'string', description: store.getTranslation('settings.tools.imagegen.edit.image_url')},
							prompt: {type: 'string', description: store.getTranslation('settings.tools.imagegen.edit.prompt')},
						},
						required: ['prompt'],
					},
				},
			],
			settings_schema: {
				type: 'object',
				required: ['image_model_id'],
				properties: {
					image_model_id: {
						type: 'string',
						title: store.getTranslation('settings.tools.image_model_title'),
						format: 'model-picker',
						description: store.getTranslation('settings.tools.imagegen.settings.description'),
					},
				},
			},
		},
	]);
}
