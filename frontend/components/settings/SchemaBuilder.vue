<template>
	<div class="space-y-4">
		<Tabs v-model="activeMode">
			<TabsList class="grid w-full grid-cols-2">
				<TabsTrigger value="visual">
					<Settings2 class="h-4 w-4 mr-2" />
					Visual
				</TabsTrigger>
				<TabsTrigger value="code">
					<Code class="h-4 w-4 mr-2" />
					Code
				</TabsTrigger>
			</TabsList>

			<TabsContent value="visual" class="space-y-3 pt-2">
				<div v-for="(prop, index) in properties" :key="index" class="border border-border rounded-lg p-3 space-y-3">
					<div class="flex items-center justify-between">
						<div class="flex items-center gap-2">
							<GripVertical class="h-4 w-4 text-muted-foreground cursor-move" />
							<Input v-model="prop.name" placeholder="property_name" class="w-40 h-8 text-sm" />
						</div>
						<ShadButton variant="ghost" size="sm" @click="removeProperty(index)">
							<X class="h-4 w-4 text-muted-foreground" />
						</ShadButton>
					</div>

					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1">
							<Label class="text-xs">{{ store.getTranslation('settings.schema_builder.type') }}</Label>
							<ShadSelect v-model="prop.type">
								<ShadSelectTrigger class="h-8">
									<ShadSelectValue />
								</ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="string">String</ShadSelectItem>
									<ShadSelectItem value="number">Number</ShadSelectItem>
									<ShadSelectItem value="integer">Integer</ShadSelectItem>
									<ShadSelectItem value="boolean">Boolean</ShadSelectItem>
									<ShadSelectItem value="array">Array</ShadSelectItem>
									<ShadSelectItem value="object">Object</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
						<div class="space-y-1">
							<Label class="text-xs">{{ store.getTranslation('settings.schema_builder.default') }}</Label>
							<Input v-model="prop.default" :placeholder="store.getTranslation('settings.schema_builder.default_placeholder')" class="h-8 text-sm" />
						</div>
					</div>

					<div class="space-y-1">
						<Label class="text-xs">{{ store.getTranslation('settings.schema_builder.description') }}</Label>
						<Input v-model="prop.description" :placeholder="store.getTranslation('settings.schema_builder.description_placeholder')" class="h-8 text-sm" />
					</div>

					<div class="space-y-1">
						<Label class="text-xs">{{ store.getTranslation('settings.schema_builder.options') }}</Label>
						<Input v-model="prop.enumString" :placeholder="store.getTranslation('settings.schema_builder.options_placeholder')" class="h-8 text-sm" />
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.schema_builder.options_hint') }}</p>
					</div>

					<div class="flex items-center gap-4">
						<label class="flex items-center gap-2 text-sm">
							<input type="checkbox" v-model="prop.required" class="rounded border-border" />
							{{ store.getTranslation('settings.schema_builder.required') }}
						</label>
						<label v-if="isSettings" class="flex items-center gap-2 text-sm">
							<input type="checkbox" v-model="prop.secret" class="rounded border-border" />
							{{ store.getTranslation('settings.schema_builder.secret') }}
						</label>
					</div>
				</div>

				<ShadButton variant="outline" size="sm" class="w-full gap-2" @click="addProperty">
					<Plus class="h-4 w-4" />
					{{ store.getTranslation('settings.schema_builder.add_property') }}
				</ShadButton>
			</TabsContent>

			<TabsContent value="code" class="pt-2">
				<Textarea v-model="jsonCode" :placeholder="defaultPlaceholder" rows="8" class="font-mono text-sm" @blur="syncFromJson" />
				<p class="text-xs text-muted-foreground mt-1">{{ store.getTranslation('settings.schema_builder.json_format') }}</p>
			</TabsContent>
		</Tabs>
	</div>
</template>

<script setup lang="ts">
import {ref, watch, computed} from 'vue';
import {Plus, X, GripVertical, Settings2, Code} from 'lucide-vue-next';
import {Tabs, TabsList, TabsTrigger, TabsContent} from '@/components/ui/tabs';
import {Input} from '@/components/ui/input';
import {Textarea} from '@/components/ui/textarea';
import {Label} from '@/components/ui/label';
import {useMainStore} from '~/stores';

const store = useMainStore();

interface PropertyDef {
	name: string;
	type: string;
	description: string;
	default: string;
	required: boolean;
	secret: boolean;
	enumString: string;
}

const props = defineProps<{
	isSettings?: boolean;
}>();

const modelValue = defineModel<string>({default: ''});
const activeMode = ref('visual');
const jsonCode = ref('');

const properties = ref<PropertyDef[]>([]);

const defaultPlaceholder = computed(() =>
	props.isSettings
		? '{"type": "object", "properties": {"api_key": {"type": "string", "secret": true}}}'
		: '{"type": "object", "properties": {"query": {"type": "string"}}}'
);

function parseSchema(json: string): PropertyDef[] {
	try {
		const schema = JSON.parse(json || '{}');
		if (!schema.properties) return [];

		const required = schema.required || [];
		return Object.entries(schema.properties).map(([name, propSchema]: [string, any]) => ({
			name,
			type: propSchema.type || 'string',
			description: propSchema.description || '',
			default: propSchema.default !== undefined ? String(propSchema.default) : '',
			required: required.includes(name),
			secret: propSchema.secret || false,
			enumString: propSchema.enum ? propSchema.enum.join(', ') : '',
		}));
	} catch {
		return [];
	}
}

function buildSchema(): string {
	if (properties.value.length === 0) {
		return JSON.stringify({type: 'object', properties: {}}, null, 2);
	}

	const schema: any = {
		type: 'object',
		properties: {},
	};

	const required: string[] = [];

	for (const prop of properties.value) {
		if (!prop.name) continue;

		const propSchema: any = {
			type: prop.type || 'string',
		};

		if (prop.description) propSchema.description = prop.description;

		if (prop.default) {
			if (prop.type === 'number' || prop.type === 'integer') {
				propSchema.default = Number(prop.default);
			} else if (prop.type === 'boolean') {
				propSchema.default = prop.default === 'true';
			} else {
				propSchema.default = prop.default;
			}
		}

		if (prop.enumString) {
			propSchema.enum = prop.enumString
				.split(',')
				.map(s => s.trim())
				.filter(Boolean);
		}

		if (props.isSettings && prop.secret) {
			propSchema.secret = true;
		}

		if (prop.required) {
			required.push(prop.name);
		}

		schema.properties[prop.name] = propSchema;
	}

	if (required.length > 0) {
		schema.required = required;
	}

	return JSON.stringify(schema, null, 2);
}

function addProperty() {
	properties.value.push({
		name: '',
		type: 'string',
		description: '',
		default: '',
		required: false,
		secret: false,
		enumString: '',
	});
}

function removeProperty(index: number) {
	properties.value.splice(index, 1);
}

function syncFromJson() {
	properties.value = parseSchema(jsonCode.value);
}

watch(
	modelValue,
	val => {
		if (val && val !== jsonCode.value) {
			jsonCode.value = val;
			properties.value = parseSchema(val);
		}
	},
	{immediate: true}
);

watch(
	properties,
	() => {
		if (activeMode.value === 'visual') {
			const json = buildSchema();
			jsonCode.value = json;
			modelValue.value = json;
		}
	},
	{deep: true}
);

watch(jsonCode, val => {
	if (activeMode.value === 'code') {
		modelValue.value = val;
	}
});

watch(activeMode, newMode => {
	if (newMode === 'visual') {
		properties.value = parseSchema(jsonCode.value);
	} else {
		jsonCode.value = buildSchema();
	}
});
</script>
