<template>
	<div class="w-full flex flex-col lg:max-h-[calc(100dvh-12rem)]">
		<!-- Top Header Bar -->
		<div class="sticky top-0 z-10 bg-background/95 backdrop-blur-sm border-b border-border px-3 py-3 flex items-center gap-3 flex-shrink-0">
			<ShadButton variant="ghost" size="icon" @click="handleBack">
				<ArrowLeft class="h-4 w-4" />
			</ShadButton>

			<div class="flex items-center gap-3 flex-1 min-w-0">
				<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-muted flex-shrink-0 overflow-hidden">
					<img v-if="model?.icon" :src="model.icon" class="h-full w-full object-cover" alt="Model icon" />
					<div
						v-else-if="providerIcon?.type === 'svg'"
						v-html="providerIcon.icon"
						class="h-5 w-5 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full text-muted-foreground"
					/>
					<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" alt="Provider icon" class="h-5 w-5" />
					<Bot v-else class="h-5 w-5 text-muted-foreground" />
				</div>
				<div class="min-w-0">
					<h2 class="text-sm font-semibold text-foreground truncate">
						{{ model ? model.display_name : store.getTranslation('settings.models.editor.loading') }}
					</h2>
					<p v-if="model" class="text-xs text-muted-foreground truncate">{{ model.provider_name }} &bull; {{ model.model_id }}</p>
				</div>
			</div>

			<span v-if="hasUnsavedChanges" class="hidden sm:flex items-center gap-1.5 text-xs text-amber-500 flex-shrink-0">
				<AlertCircle class="h-3.5 w-3.5" />
				{{ store.getTranslation('settings.models.editor.unsaved_changes') }}
			</span>

			<div v-if="hasUnsavedChanges" class="flex items-center gap-2">
				<ShadButton size="sm" class="gap-1.5" :disabled="saving || loading || !model" @click="saveModel">
					<Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
					<Save v-else class="h-4 w-4" />
					{{ store.getTranslation('common.save') }}
				</ShadButton>
			</div>
		</div>

		<!-- Stepper Navigation -->
		<div v-if="model && !loading" class="border-b border-border px-3 py-3 flex-shrink-0 overflow-auto">
			<nav class="flex items-center justify-center gap-1 sm:gap-2 w-full">
				<button v-for="(step, index) in steps" :key="step.key" class="flex items-center gap-1.5 group" @click="currentStep = index">
					<span
						class="flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium transition-colors"
						:class="
							currentStep === index
								? 'bg-primary text-primary-foreground'
								: currentStep > index
									? 'bg-primary/20 text-primary'
									: 'bg-muted text-muted-foreground'
						"
					>
						<Check v-if="currentStep > index" class="h-3.5 w-3.5" />
						<span v-else>{{ index + 1 }}</span>
					</span>
					<span class="text-xs font-medium transition-colors hidden sm:inline" :class="currentStep === index ? 'text-foreground' : 'text-muted-foreground'">
						{{ store.getTranslation(step.label) }}
					</span>
					<ChevronRight v-if="index < steps.length - 1" class="h-3.5 w-3.5 text-muted-foreground mx-0.5" />
				</button>
			</nav>
		</div>

		<!-- Loading State -->
		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground flex-1">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<!-- Not Found State -->
		<div v-else-if="!model" class="flex items-center justify-center py-12 text-muted-foreground flex-1">
			<p>{{ store.getTranslation('settings.models.not_found') }}</p>
		</div>

		<!-- Step Content -->
		<div v-else class="flex-1 overflow-y-auto px-4 py-5">
			<!-- Step 1: General Info -->
			<div v-show="currentStep === 0" class="space-y-5">
				<div>
					<h3 class="text-base font-semibold text-foreground mb-1">
						{{ store.getTranslation('settings.models.editor.general_info') }}
					</h3>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.general_info_desc') }}
					</p>
				</div>

				<div class="space-y-4">
					<!-- Display Name -->
					<div class="space-y-1.5">
						<ShadLabel for="display-name">
							{{ store.getTranslation('settings.models.editor.display_name') }}
						</ShadLabel>
						<ShadInput
							id="display-name"
							v-model="formData.display_name"
							:placeholder="store.getTranslation('settings.models.editor.display_name_placeholder')"
						/>
					</div>

					<!-- Model ID (Read Only) -->
					<div class="space-y-1.5">
						<ShadLabel>
							{{ store.getTranslation('settings.models.editor.model_id') }}
						</ShadLabel>
						<div class="flex items-center gap-2">
							<ShadInput :model-value="`${model.provider.name} • ${model.model_id}`" disabled class="flex-1 font-mono text-xs" />
							<ShadButton variant="outline" size="icon" class="flex-shrink-0" @click="copyModelId">
								<Copy class="h-3.5 w-3.5" />
							</ShadButton>
						</div>
					</div>

					<!-- Description -->
					<div class="space-y-1.5">
						<ShadLabel for="description">
							{{ store.getTranslation('settings.models.editor.description') }}
						</ShadLabel>
						<ShadTextarea
							id="description"
							v-model="formData.description"
							:placeholder="store.getTranslation('settings.models.editor.description_placeholder')"
							rows="3"
						/>
					</div>

					<!-- Enable Model -->
					<div class="flex items-center justify-between rounded-lg border border-border p-3">
						<div class="space-y-0.5">
							<ShadLabel>
								{{ store.getTranslation('settings.models.editor.enable_model') }}
							</ShadLabel>
							<p class="text-xs text-muted-foreground">
								{{ store.getTranslation('settings.models.editor.enable_model_desc') }}
							</p>
						</div>
						<ShadSwitch v-model:model-value="formData.is_enabled" />
					</div>

					<!-- Change Model Icon -->
					<div class="space-y-1.5">
						<ShadLabel>
							{{ store.getTranslation('settings.models.editor.change_icon') }}
						</ShadLabel>
						<div class="space-y-3">
							<div class="flex items-center gap-3">
								<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-muted overflow-hidden border border-border">
									<img v-if="formData.icon" :src="formData.icon" class="h-full w-full object-cover" alt="Model icon" />
									<Bot v-else class="h-6 w-6 text-muted-foreground" />
								</div>
								<div class="flex gap-2">
									<ShadButton variant="outline" size="sm" class="gap-1.5" @click="triggerIconUpload">
										<Upload class="h-3.5 w-3.5" />
										{{ store.getTranslation('settings.models.editor.upload') }}
									</ShadButton>
									<ShadButton v-if="formData.icon" variant="ghost" size="sm" class="gap-1.5 text-destructive" @click="formData.icon = ''">
										<Trash2 class="h-3.5 w-3.5" />
										{{ store.getTranslation('common.remove') }}
									</ShadButton>
								</div>
								<input ref="iconInputRef" type="file" accept="image/*" class="hidden" @change="handleIconUpload" />
							</div>
							<div class="space-y-1.5">
								<ShadLabel for="icon-url">
									{{ store.getTranslation('settings.models.editor.icon_url') }}
								</ShadLabel>
								<ShadInput
									id="icon-url"
									v-model="formData.icon"
									type="url"
									:placeholder="store.getTranslation('settings.models.editor.icon_url_placeholder')"
								/>
							</div>
						</div>
					</div>
				</div>
			</div>

			<!-- Step 2: Core Prompt -->
			<div v-show="currentStep === 1" class="space-y-5">
				<div>
					<h3 class="text-base font-semibold text-foreground mb-1">
						{{ store.getTranslation('settings.models.editor.core_prompt') }}
					</h3>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.core_prompt_desc') }}
					</p>
				</div>

				<div class="space-y-4">
					<div class="space-y-1.5">
						<div class="flex items-center justify-between">
							<ShadLabel for="system-prompt">
								{{ store.getTranslation('settings.models.editor.system_prompt') }}
							</ShadLabel>
							<span class="text-xs text-muted-foreground">
								{{ formData.system_prompt?.length || 0 }} {{ store.getTranslation('settings.models.editor.characters') }}
							</span>
						</div>
						<ShadTextarea
							id="system-prompt"
							v-model="formData.system_prompt"
							:placeholder="store.getTranslation('settings.models.editor.system_prompt_placeholder')"
							rows="12"
							class="font-mono text-sm"
						/>
						<p class="text-xs text-muted-foreground">
							{{ store.getTranslation('settings.models.editor.system_prompt_hint') }}
						</p>
					</div>
				</div>
			</div>

			<!-- Step 3: Parameters -->
			<div v-show="currentStep === 2" class="space-y-6">
				<div>
					<h3 class="text-base font-semibold text-foreground mb-1">
						{{ store.getTranslation('settings.models.editor.parameters') }}
					</h3>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.parameters_desc') }}
					</p>
				</div>

				<!-- Group: Core Sampling -->
				<div class="space-y-3">
					<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.param_group_core') }}
					</p>
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
						<!-- Temperature -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.temperature !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.temperature !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.temperature') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.temperature_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.temperature !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ formData.sampling.temperature.toFixed(2) }}
									</span>
									<ShadButton
										variant="ghost"
										size="icon"
										class="h-6 w-6"
										@click="toggleParam('temperature', 0.7)"
									>
										<Minus v-if="formData.sampling.temperature !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.temperature !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="temperatureSlider" :min="0" :max="2" :step="0.01" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>0 — {{ store.getTranslation('settings.models.editor.temperature_label_low') }}</span>
									<span>{{ store.getTranslation('settings.models.editor.temperature_label_high') }} — 2</span>
								</div>
							</div>
						</div>

						<!-- Max Tokens -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.max_tokens !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.max_tokens !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.max_tokens') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.max_tokens_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.max_tokens !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ formData.sampling.max_tokens.toLocaleString() }}
									</span>
									<ShadButton
										variant="ghost"
										size="icon"
										class="h-6 w-6"
										@click="toggleParam('max_tokens', 4096)"
									>
										<Minus v-if="formData.sampling.max_tokens !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.max_tokens !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="maxTokensSlider" :min="256" :max="128000" :step="256" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>256</span>
									<span>128 000</span>
								</div>
							</div>
						</div>
					</div>
				</div>

				<!-- Group: Token Sampling -->
				<div class="space-y-3">
					<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.param_group_token') }}
					</p>
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
						<!-- Top P -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.top_p !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.top_p !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.top_p') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.top_p_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.top_p !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ formData.sampling.top_p.toFixed(2) }}
									</span>
									<ShadButton
										variant="ghost"
										size="icon"
										class="h-6 w-6"
										@click="toggleParam('top_p', 1)"
									>
										<Minus v-if="formData.sampling.top_p !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.top_p !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="topPSlider" :min="0" :max="1" :step="0.01" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>0</span>
									<span>1</span>
								</div>
							</div>
						</div>

						<!-- Top K -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.top_k !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.top_k !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.top_k') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.top_k_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.top_k !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ formData.sampling.top_k }}
									</span>
									<ShadButton
										variant="ghost"
										size="icon"
										class="h-6 w-6"
										@click="toggleParam('top_k', 40)"
									>
										<Minus v-if="formData.sampling.top_k !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.top_k !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="topKSlider" :min="1" :max="500" :step="1" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>1</span>
									<span>500</span>
								</div>
							</div>
						</div>
					</div>
				</div>

				<!-- Group: Penalties -->
				<div class="space-y-3">
					<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.param_group_penalties') }}
					</p>
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
						<!-- Frequency Penalty -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.frequency_penalty !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.frequency_penalty !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.frequency_penalty') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.frequency_penalty_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.frequency_penalty !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ (formData.sampling.frequency_penalty >= 0 ? '+' : '') + formData.sampling.frequency_penalty.toFixed(2) }}
									</span>
									<ShadButton
										variant="ghost"
										size="icon"
										class="h-6 w-6"
										@click="toggleParam('frequency_penalty', 0)"
									>
										<Minus v-if="formData.sampling.frequency_penalty !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.frequency_penalty !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="frequencyPenaltySlider" :min="-2" :max="2" :step="0.01" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>-2</span>
									<span>0</span>
									<span>+2</span>
								</div>
							</div>
						</div>

						<!-- Presence Penalty -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.presence_penalty !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.presence_penalty !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.presence_penalty') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.presence_penalty_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.presence_penalty !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ (formData.sampling.presence_penalty >= 0 ? '+' : '') + formData.sampling.presence_penalty.toFixed(2) }}
									</span>
									<ShadButton
										variant="ghost"
										size="icon"
										class="h-6 w-6"
										@click="toggleParam('presence_penalty', 0)"
									>
										<Minus v-if="formData.sampling.presence_penalty !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.presence_penalty !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="presencePenaltySlider" :min="-2" :max="2" :step="0.01" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>-2</span>
									<span>0</span>
									<span>+2</span>
								</div>
							</div>
						</div>
					</div>
				</div>

				<!-- Group: Reasoning (conditional) -->
				<div v-if="hasEffortReasoning || hasBudgetReasoning" class="space-y-3">
					<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.param_group_reasoning') }}
					</p>
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
						<!-- Reasoning Effort -->
						<div
							v-if="hasEffortReasoning"
							class="rounded-lg border transition-colors"
							:class="formData.sampling.reasoning_effort !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.reasoning_effort !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.reasoning_effort') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.reasoning_effort_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<ShadButton
									variant="ghost"
									size="icon"
									class="h-6 w-6"
									@click="toggleParam('reasoning_effort', availableEffortLevels[1] ?? availableEffortLevels[0] ?? 'medium')"
								>
									<Minus v-if="formData.sampling.reasoning_effort !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
									<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
								</ShadButton>
							</div>
							<div v-if="formData.sampling.reasoning_effort !== null" class="px-3 pb-3">
								<ShadSelect v-model="formData.sampling.reasoning_effort">
									<ShadSelectTrigger class="h-8 text-xs w-full">
										<ShadSelectValue :placeholder="store.getTranslation('settings.models.editor.reasoning_effort_placeholder')" />
									</ShadSelectTrigger>
									<ShadSelectContent>
										<ShadSelectGroup>
											<ShadSelectItem v-for="effort in availableEffortLevels" :key="effort" :value="effort" class="text-xs">
												{{ effortLabel(effort) }}
											</ShadSelectItem>
										</ShadSelectGroup>
									</ShadSelectContent>
								</ShadSelect>
							</div>
						</div>

						<!-- Reasoning Budget Tokens -->
						<div
							v-if="hasBudgetReasoning"
							class="rounded-lg border transition-colors"
							:class="formData.sampling.reasoning_budget_tokens !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.reasoning_budget_tokens !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.reasoning_budget') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.reasoning_budget_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.reasoning_budget_tokens !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ Math.round(formData.sampling.reasoning_budget_tokens / 1024) }}K
									</span>
									<ShadButton
										variant="ghost"
										size="icon"
										class="h-6 w-6"
										@click="toggleParam('reasoning_budget_tokens', reasoningBudgetRange.min)"
									>
										<Minus v-if="formData.sampling.reasoning_budget_tokens !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.reasoning_budget_tokens !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="reasoningBudgetSlider" :min="reasoningBudgetRange.min" :max="reasoningBudgetRange.max" :step="1024" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>{{ Math.round(reasoningBudgetRange.min / 1024) }}K</span>
									<span>{{ Math.round(reasoningBudgetRange.max / 1024) }}K</span>
								</div>
							</div>
						</div>
					</div>
				</div>

				<!-- Group: Advanced -->
				<div class="space-y-3">
					<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.param_group_advanced') }}
					</p>
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
						<!-- Seed -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.seed !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3" :class="formData.sampling.seed !== null ? 'pb-2' : 'pb-3'">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.seed !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.seed') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.seed_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<ShadButton variant="ghost" size="icon" class="h-6 w-6" @click="toggleParam('seed', 42)">
									<Minus v-if="formData.sampling.seed !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
									<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
								</ShadButton>
							</div>
							<div v-if="formData.sampling.seed !== null" class="px-3 pb-3">
								<ShadInput
									v-model.number="formData.sampling.seed"
									type="number"
									min="0"
									step="1"
									class="h-8 text-xs font-mono"
									:placeholder="store.getTranslation('settings.models.editor.seed_placeholder')"
								/>
							</div>
						</div>

						<!-- Stop Sequences -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.stop !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3" :class="formData.sampling.stop !== null ? 'pb-2' : 'pb-3'">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.stop !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.stop_sequences') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.stop_sequences_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<ShadButton variant="ghost" size="icon" class="h-6 w-6" @click="toggleStopSequences">
									<Minus v-if="formData.sampling.stop !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
									<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
								</ShadButton>
							</div>
							<div v-if="formData.sampling.stop !== null" class="px-3 pb-3 space-y-2">
								<div v-if="formData.sampling.stop.length > 0" class="flex flex-wrap gap-1">
									<div
										v-for="(seq, idx) in formData.sampling.stop"
										:key="idx"
										class="flex items-center gap-1 rounded-md bg-muted px-2 py-0.5 text-xs font-mono"
									>
										<span>{{ seq }}</span>
										<button class="text-muted-foreground hover:text-foreground transition-colors" @click="removeStopSequence(idx)">
											<X class="h-3 w-3" />
										</button>
									</div>
								</div>
								<div class="flex gap-1.5">
									<ShadInput
										v-model="newStopSequence"
										class="h-8 text-xs font-mono flex-1"
										:placeholder="store.getTranslation('settings.models.editor.stop_sequences_placeholder')"
										@keydown.enter.prevent="addStopSequence"
									/>
									<ShadButton variant="outline" size="icon" class="h-8 w-8 flex-shrink-0" :disabled="!newStopSequence.trim()" @click="addStopSequence">
										<Plus class="h-3.5 w-3.5" />
									</ShadButton>
								</div>
							</div>
						</div>

						<!-- Parallel Tool Calls -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.parallel_tool_calls !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 py-3">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.parallel_tool_calls !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.parallel_tool_calls') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.parallel_tool_calls_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<ShadSwitch
										v-if="formData.sampling.parallel_tool_calls !== null"
										v-model:model-value="formData.sampling.parallel_tool_calls"
									/>
									<ShadButton variant="ghost" size="icon" class="h-6 w-6" @click="toggleParam('parallel_tool_calls', true)">
										<Minus v-if="formData.sampling.parallel_tool_calls !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
						</div>

						<!-- Logprobs -->
						<div
							class="rounded-lg border transition-colors"
							:class="formData.sampling.logprobs !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 py-3">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.logprobs !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.logprobs') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.logprobs_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<ShadSwitch
										v-if="formData.sampling.logprobs !== null"
										v-model:model-value="formData.sampling.logprobs"
									/>
									<ShadButton variant="ghost" size="icon" class="h-6 w-6" @click="toggleParam('logprobs', false)">
										<Minus v-if="formData.sampling.logprobs !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
						</div>

						<!-- Top Logprobs (only when logprobs enabled) -->
						<div
							v-if="formData.sampling.logprobs"
							class="rounded-lg border transition-colors"
							:class="formData.sampling.top_logprobs !== null ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'"
						>
							<div class="flex items-center justify-between px-3 pt-3 pb-2">
								<div class="flex items-center gap-1.5">
									<span class="text-sm font-medium" :class="formData.sampling.top_logprobs !== null ? 'text-foreground' : 'text-muted-foreground'">
										{{ store.getTranslation('settings.models.editor.top_logprobs') }}
									</span>
									<ShadTooltipProvider>
										<ShadTooltip>
											<ShadTooltipTrigger as-child>
												<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
											</ShadTooltipTrigger>
											<ShadTooltipContent side="top" class="max-w-xs text-xs">
												{{ store.getTranslation('settings.models.editor.top_logprobs_tooltip') }}
											</ShadTooltipContent>
										</ShadTooltip>
									</ShadTooltipProvider>
								</div>
								<div class="flex items-center gap-2">
									<span v-if="formData.sampling.top_logprobs !== null" class="text-sm font-mono font-medium text-primary tabular-nums">
										{{ formData.sampling.top_logprobs }}
									</span>
									<ShadButton variant="ghost" size="icon" class="h-6 w-6" @click="toggleParam('top_logprobs', 5)">
										<Minus v-if="formData.sampling.top_logprobs !== null" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
										<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</ShadButton>
								</div>
							</div>
							<div v-if="formData.sampling.top_logprobs !== null" class="px-3 pb-3 space-y-2">
								<ShadSlider v-model="topLogprobsSlider" :min="0" :max="20" :step="1" />
								<div class="flex justify-between text-[10px] text-muted-foreground">
									<span>0</span>
									<span>20</span>
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>

	<!-- Unsaved Changes Dialog -->
	<ShadDialog v-model:open="showUnsavedDialog">
		<ShadDialogContent class="sm:max-w-md">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('settings.models.editor.unsaved_dialog_title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('settings.models.editor.unsaved_dialog_desc') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<ShadDialogFooter class="flex-col-reverse sm:flex-row gap-2 sm:gap-0">
				<ShadButton variant="outline" @click="showUnsavedDialog = false">
					{{ store.getTranslation('settings.models.editor.keep_editing') }}
				</ShadButton>
				<ShadButton variant="destructive" @click="confirmGoBack">
					{{ store.getTranslation('settings.models.editor.discard_changes') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {ArrowLeft, Loader2, Bot, AlertCircle, Save, Check, ChevronRight, Copy, Upload, Trash2, Info, X, Plus, Minus} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {useNuxtApp} from '#app';

const store = useMainStore();
const iconStore = useIconsStore();
const route = useRoute();
const router = useRouter();
const {$customFetch} = useNuxtApp();

const modelId = route.params.id as string;
const model = ref<Record<string, any> | null>(null);
const loading = ref(true);
const saving = ref(false);
const showUnsavedDialog = ref(false);
const currentStep = ref(0);
const iconInputRef = ref<HTMLInputElement | null>(null);
const newStopSequence = ref('');
const originalData = ref<string>('');

const steps = [
	{key: 'general', label: 'settings.models.editor.general_info'},
	{key: 'prompt', label: 'settings.models.editor.core_prompt'},
	{key: 'parameters', label: 'settings.models.editor.parameters'},
];

type SamplingState = {
	temperature: number | null;
	max_tokens: number | null;
	top_p: number | null;
	top_k: number | null;
	frequency_penalty: number | null;
	presence_penalty: number | null;
	seed: number | null;
	stop: string[] | null;
	logprobs: boolean | null;
	top_logprobs: number | null;
	parallel_tool_calls: boolean | null;
	reasoning_effort: string | null;
	reasoning_budget_tokens: number | null;
};

const formData = reactive({
	display_name: '',
	description: '',
	is_enabled: true,
	system_prompt: '',
	icon: '',
	sampling: {
		temperature: null,
		max_tokens: null,
		top_p: null,
		top_k: null,
		frequency_penalty: null,
		presence_penalty: null,
		seed: null,
		stop: null,
		logprobs: null,
		top_logprobs: null,
		parallel_tool_calls: null,
		reasoning_effort: null,
		reasoning_budget_tokens: null,
	} as SamplingState,
});

function makeSlider(key: keyof SamplingState, fallback: number) {
	return computed({
		get: () => [(formData.sampling[key] as number | null) ?? fallback],
		set: (val: number[]) => {
			(formData.sampling as any)[key] = val[0];
		},
	});
}

const temperatureSlider = makeSlider('temperature', 0.7);
const maxTokensSlider = makeSlider('max_tokens', 4096);
const topPSlider = makeSlider('top_p', 1);
const topKSlider = makeSlider('top_k', 40);
const frequencyPenaltySlider = makeSlider('frequency_penalty', 0);
const presencePenaltySlider = makeSlider('presence_penalty', 0);
const topLogprobsSlider = makeSlider('top_logprobs', 5);
const reasoningBudgetSlider = makeSlider('reasoning_budget_tokens', 1024);

const hasUnsavedChanges = computed(() => JSON.stringify(formData) !== originalData.value);

const providerIcon = computed(() => {
	if (!model.value) return null;
	return iconStore.getProviderIcon(model.value.provider_name, model.value.model_id);
});

const modelCapabilities = computed<string[]>(() => model.value?.capabilities ?? []);

const hasEffortReasoning = computed(() => modelCapabilities.value.some(c => c.startsWith('REASONING_EFFORT_')));

const hasBudgetReasoning = computed(() => modelCapabilities.value.some(c => c.startsWith('REASONING_BUDGET_TOKENS_')));

const availableEffortLevels = computed(() => {
	const order = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'];
	return order.filter(e => modelCapabilities.value.includes(`REASONING_EFFORT_${e.toUpperCase()}`));
});

const reasoningBudgetRange = computed(() => {
	for (const cap of modelCapabilities.value) {
		const match = cap.match(/REASONING_BUDGET_TOKENS_(\d+)_(\d+)/);
		if (match) {
			return {min: parseInt(match[1] ?? '0'), max: parseInt(match[2] ?? '0')};
		}
	}
	return {min: 1024, max: 32768};
});

function effortLabel(effort: string): string {
	const map: Record<string, string> = {
		none: store.getTranslation('chat.reasoning_selector.none'),
		minimal: store.getTranslation('chat.reasoning_selector.minimal'),
		low: store.getTranslation('chat.reasoning_selector.low'),
		medium: store.getTranslation('chat.reasoning_selector.medium'),
		high: store.getTranslation('chat.reasoning_selector.high'),
		xhigh: store.getTranslation('chat.reasoning_selector.extra_high'),
	};
	return map[effort] ?? effort;
}

function toggleParam(key: keyof SamplingState, defaultValue: any) {
	if ((formData.sampling as any)[key] !== null) {
		(formData.sampling as any)[key] = null;
	} else {
		(formData.sampling as any)[key] = defaultValue;
	}
}

function toggleStopSequences() {
	formData.sampling.stop = formData.sampling.stop !== null ? null : [];
}

function addStopSequence() {
	const seq = newStopSequence.value.trim();
	if (!seq) return;
	if (!formData.sampling.stop) formData.sampling.stop = [];
	if (!formData.sampling.stop.includes(seq)) {
		formData.sampling.stop.push(seq);
	}
	newStopSequence.value = '';
}

function removeStopSequence(index: number) {
	formData.sampling.stop?.splice(index, 1);
}

onMounted(async () => {
	await loadModel();
});

function populateForm(data: Record<string, any>) {
	formData.display_name = data.display_name || '';
	formData.description = data.description || '';
	formData.is_enabled = data.is_enabled ?? true;
	formData.system_prompt = data.system_prompt || '';
	formData.icon = data.icon || '';

	const s = data.sampling ?? {};
	formData.sampling.temperature = s.temperature !== undefined ? s.temperature : null;
	formData.sampling.max_tokens = s.max_tokens !== undefined ? s.max_tokens : null;
	formData.sampling.top_p = s.top_p !== undefined ? s.top_p : null;
	formData.sampling.top_k = s.top_k !== undefined ? s.top_k : null;
	formData.sampling.frequency_penalty = s.frequency_penalty !== undefined ? s.frequency_penalty : null;
	formData.sampling.presence_penalty = s.presence_penalty !== undefined ? s.presence_penalty : null;
	formData.sampling.seed = s.seed !== undefined ? s.seed : null;
	formData.sampling.stop = Array.isArray(s.stop) && s.stop.length > 0 ? [...s.stop] : null;
	formData.sampling.logprobs = s.logprobs !== undefined ? s.logprobs : null;
	formData.sampling.top_logprobs = s.top_logprobs !== undefined ? s.top_logprobs : null;
	formData.sampling.parallel_tool_calls = s.parallel_tool_calls !== undefined ? s.parallel_tool_calls : null;
	formData.sampling.reasoning_effort = s.reasoning_effort !== undefined ? s.reasoning_effort : null;
	formData.sampling.reasoning_budget_tokens = s.reasoning_budget_tokens !== undefined ? s.reasoning_budget_tokens : null;

	originalData.value = JSON.stringify(formData);
}

async function loadModel() {
	loading.value = true;
	try {
		const res = await $customFetch(`/api/v1/admin/models/${modelId}`);
		if (res) {
			model.value = res as Record<string, any>;
			populateForm(model.value);
		}
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.not_found'), {type: 'error'});
	} finally {
		loading.value = false;
	}
}

function copyModelId() {
	if (!model.value) return;
	navigator.clipboard.writeText(model.value.model_id);
	store.toast(store.getTranslation('common.copied'), {type: 'success'});
}

function triggerIconUpload() {
	iconInputRef.value?.click();
}

function handleIconUpload(event: Event) {
	const target = event.target as HTMLInputElement;
	const file = target.files?.[0];
	if (!file) return;

	const reader = new FileReader();
	reader.onload = e => {
		formData.icon = e.target?.result as string;
	};
	reader.readAsDataURL(file);
	target.value = '';
}

function handleBack() {
	if (hasUnsavedChanges.value) {
		showUnsavedDialog.value = true;
	} else {
		router.push('/settings/models');
	}
}

function confirmGoBack() {
	showUnsavedDialog.value = false;
	router.push('/settings/models');
}

function buildSamplingPayload(): Record<string, any> | null {
	const s = formData.sampling;
	const payload: Record<string, any> = {};

	if (s.temperature !== null) payload.temperature = s.temperature;
	if (s.max_tokens !== null) payload.max_tokens = s.max_tokens;
	if (s.top_p !== null) payload.top_p = s.top_p;
	if (s.top_k !== null) payload.top_k = s.top_k;
	if (s.frequency_penalty !== null) payload.frequency_penalty = s.frequency_penalty;
	if (s.presence_penalty !== null) payload.presence_penalty = s.presence_penalty;
	if (s.seed !== null) payload.seed = s.seed;
	if (s.stop !== null) payload.stop = s.stop;
	if (s.logprobs !== null) payload.logprobs = s.logprobs;
	if (s.top_logprobs !== null) payload.top_logprobs = s.top_logprobs;
	if (s.parallel_tool_calls !== null) payload.parallel_tool_calls = s.parallel_tool_calls;
	if (s.reasoning_effort !== null) payload.reasoning_effort = s.reasoning_effort;
	if (s.reasoning_budget_tokens !== null) payload.reasoning_budget_tokens = s.reasoning_budget_tokens;

	return Object.keys(payload).length > 0 ? payload : null;
}

async function saveModel() {
	if (!model.value) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/admin/models/${modelId}`, {
			method: 'PUT',
			body: {
				display_name: formData.display_name,
				is_enabled: formData.is_enabled,
				system_prompt: formData.system_prompt,
				sampling: buildSamplingPayload(),
				icon: formData.icon,
				description: formData.description,
			},
		});

		store.toast(store.getTranslation('settings.models.save_success'), {type: 'success'});
		router.push('/settings/models');
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.save_error'), {type: 'error'});
	} finally {
		saving.value = false;
	}
}
</script>
