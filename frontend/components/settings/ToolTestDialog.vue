<template>
	<Dialog v-model:open="open">
		<DialogContent class="sm:max-w-[600px] max-h-[80vh] flex flex-col">
			<DialogHeader>
				<DialogTitle class="flex items-center gap-3">
					<Play class="h-5 w-5 text-primary" />
					<span>Test {{ tool?.display_name || tool?.name }}</span>
				</DialogTitle>
				<DialogDescription> Enter input values to test this tool </DialogDescription>
			</DialogHeader>

			<Tabs v-model="activeTab" class="flex-1 min-h-0 flex flex-col">
				<TabsList class="grid w-full grid-cols-2">
					<TabsTrigger value="form">Form</TabsTrigger>
					<TabsTrigger value="json">JSON</TabsTrigger>
				</TabsList>

				<TabsContent value="form" class="flex-1 overflow-y-auto space-y-4 py-4">
					<div v-if="schemaProperties.length === 0" class="text-sm text-muted-foreground text-center py-8">This tool has no input parameters</div>
					<div v-for="prop in schemaProperties" :key="prop.name" class="space-y-2">
						<div class="flex items-center gap-2">
							<Label :for="`input-${prop.name}`" class="text-sm font-medium">
								{{ prop.name }}
								<span v-if="prop.required" class="text-destructive">*</span>
							</Label>
							<span v-if="prop.type" class="text-xs text-muted-foreground bg-muted px-1.5 py-0.5 rounded">
								{{ prop.type }}
							</span>
						</div>
						<p v-if="prop.description" class="text-xs text-muted-foreground">{{ prop.description }}</p>

						<!-- Enum/Select field -->
						<ShadSelect v-if="prop.enum" v-model="formInputs[prop.name]">
							<ShadSelectTrigger :id="`input-${prop.name}`">
								<ShadSelectValue :placeholder="prop.default ?? 'Select...'" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="option in prop.enum" :key="option" :value="option">
									{{ option }}
								</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>

						<!-- Boolean field -->
						<div v-else-if="prop.type === 'boolean'" class="flex items-center gap-2">
							<Switch :id="`input-${prop.name}`" v-model:checked="formInputs[prop.name]" />
							<Label :for="`input-${prop.name}`" class="text-sm">{{ formInputs[prop.name] ? 'True' : 'False' }}</Label>
						</div>

						<!-- Number/Integer field -->
						<Input
							v-else-if="prop.type === 'number' || prop.type === 'integer'"
							:id="`input-${prop.name}`"
							v-model.number="formInputs[prop.name]"
							type="number"
							:step="prop.type === 'integer' ? 1 : 0.1"
							:placeholder="prop.default?.toString() ?? ''"
						/>

						<!-- Array field (comma-separated) -->
						<div v-else-if="prop.type === 'array'" class="space-y-1">
							<Input :id="`input-${prop.name}`" v-model="formInputs[prop.name]" type="text" placeholder="Comma-separated values" />
							<p class="text-xs text-muted-foreground">Enter values separated by commas</p>
						</div>

						<!-- Object field (JSON) -->
						<Textarea
							v-else-if="prop.type === 'object'"
							:id="`input-${prop.name}`"
							v-model="formInputs[prop.name]"
							:placeholder="prop.default ? JSON.stringify(prop.default) : '{}'"
							rows="3"
							class="font-mono text-sm"
						/>

						<!-- String field (default) -->
						<Input v-else :id="`input-${prop.name}`" v-model="formInputs[prop.name]" type="text" :placeholder="prop.default ?? ''" />
					</div>
				</TabsContent>

				<TabsContent value="json" class="flex-1 overflow-y-auto py-4">
					<Textarea v-model="jsonInput" placeholder="{}" rows="10" class="font-mono text-sm h-full" />
				</TabsContent>
			</Tabs>

			<!-- Results area -->
			<div v-if="testResult" class="border-t border-border pt-4 space-y-2">
				<div class="flex items-center gap-2">
					<component :is="testResult.success ? CheckCircle : XCircle" class="h-4 w-4" :class="testResult.success ? 'text-green-500' : 'text-destructive'" />
					<span class="text-sm font-medium" :class="testResult.success ? 'text-green-500' : 'text-destructive'">
						{{ testResult.success ? 'Success' : 'Failed' }}
					</span>
					<span class="text-xs text-muted-foreground">{{ testResult.execution_ms }}ms</span>
				</div>
				<div v-if="testResult.error" class="text-sm text-destructive bg-destructive/10 p-2 rounded">
					{{ testResult.error }}
				</div>
				<details v-if="testResult.output" class="text-sm">
					<summary class="cursor-pointer text-muted-foreground hover:text-foreground">View output</summary>
					<pre class="h-[200px] mt-2 p-2 bg-muted rounded text-xs overflow-auto">{{ JSON.stringify(testResult.output, null, 2) }}</pre>
				</details>
			</div>

			<DialogFooter>
				<ShadButton variant="outline" @click="open = false">Close</ShadButton>
				<ShadButton @click="runTest" :disabled="testing">
					<Loader2 v-if="testing" class="h-4 w-4 animate-spin mr-2" />
					<Play v-else class="h-4 w-4 mr-2" />
					Run Test
				</ShadButton>
			</DialogFooter>
		</DialogContent>
	</Dialog>
</template>

<script setup lang="ts">
import {ref, computed, watch} from 'vue';
import {Play, Loader2, CheckCircle, XCircle} from 'lucide-vue-next';
import {Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Tabs, TabsList, TabsTrigger, TabsContent} from '@/components/ui/tabs';
import {Input} from '@/components/ui/input';
import {Textarea} from '@/components/ui/textarea';
import {Label} from '@/components/ui/label';
import {Switch} from '@/components/ui/switch';

const {$customFetch} = useNuxtApp();

interface Tool {
	id: string;
	name: string;
	display_name?: string;
	input_schema: any;
}

interface SchemaProperty {
	name: string;
	type?: string;
	description?: string;
	required: boolean;
	default?: any;
	enum?: string[];
}

interface TestResult {
	success: boolean;
	output?: any;
	error?: string;
	execution_ms: number;
}

const props = defineProps<{
	tool: Tool | null;
}>();

const open = defineModel<boolean>('open', {default: false});
const activeTab = ref('form');
const testing = ref(false);
const testResult = ref<TestResult | null>(null);
const formInputs = ref<Record<string, any>>({});
const jsonInput = ref('{}');

// Parse the input schema to extract properties
const schemaProperties = computed<SchemaProperty[]>(() => {
	if (!props.tool?.input_schema?.properties) return [];

	const schema = props.tool.input_schema;
	const required = schema.required || [];

	return Object.entries(schema.properties).map(([name, propSchema]: [string, any]) => ({
		name,
		type: propSchema.type,
		description: propSchema.description,
		required: required.includes(name),
		default: propSchema.default,
		enum: propSchema.enum,
	}));
});

// Sync form inputs to JSON when in form mode
watch(
	formInputs,
	inputs => {
		if (activeTab.value === 'form') {
			const cleaned: Record<string, any> = {};
			for (const [key, value] of Object.entries(inputs)) {
				if (value !== undefined && value !== '' && value !== null) {
					const prop = schemaProperties.value.find(p => p.name === key);
					if (prop?.type === 'array' && typeof value === 'string') {
						cleaned[key] = value
							.split(',')
							.map(s => s.trim())
							.filter(Boolean);
					} else if (prop?.type === 'object' && typeof value === 'string') {
						try {
							cleaned[key] = JSON.parse(value);
						} catch {
							cleaned[key] = value;
						}
					} else {
						cleaned[key] = value;
					}
				}
			}
			jsonInput.value = JSON.stringify(cleaned, null, 2);
		}
	},
	{deep: true}
);

// Sync JSON to form inputs when switching tabs
watch(activeTab, newTab => {
	if (newTab === 'form') {
		try {
			const parsed = JSON.parse(jsonInput.value);
			for (const prop of schemaProperties.value) {
				if (parsed[prop.name] !== undefined) {
					if (prop.type === 'array' && Array.isArray(parsed[prop.name])) {
						formInputs.value[prop.name] = parsed[prop.name].join(', ');
					} else if (prop.type === 'object' && typeof parsed[prop.name] === 'object') {
						formInputs.value[prop.name] = JSON.stringify(parsed[prop.name]);
					} else {
						formInputs.value[prop.name] = parsed[prop.name];
					}
				}
			}
		} catch {
			// Invalid JSON, don't update form
		}
	}
});

// Reset when tool changes
watch(
	() => props.tool,
	tool => {
		testResult.value = null;
		formInputs.value = {};
		jsonInput.value = '{}';

		// Initialize with defaults
		if (tool?.input_schema?.properties) {
			for (const [name, propSchema] of Object.entries(tool.input_schema.properties) as [string, any][]) {
				if (propSchema.default !== undefined) {
					formInputs.value[name] = propSchema.default;
				}
			}
		}
	},
	{immediate: true}
);

async function runTest() {
	if (!props.tool) return;

	testing.value = true;
	testResult.value = null;

	try {
		let input: any;
		try {
			input = JSON.parse(jsonInput.value);
		} catch {
			testResult.value = {
				success: false,
				error: 'Invalid JSON input',
				execution_ms: 0,
			};
			return;
		}

		const result = await $customFetch<TestResult>(`/api/v1/tools/${props.tool.id}/test`, {
			method: 'POST',
			body: {input},
		});

		testResult.value = result;
	} catch (e: any) {
		testResult.value = {
			success: false,
			error: e.message || 'Test request failed',
			execution_ms: 0,
		};
	} finally {
		testing.value = false;
	}
}
</script>
