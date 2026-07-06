<template>
	<div class="w-full">
		<!-- Header + date filter -->
		<div class="mb-6 flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ title }}</h2>
				<p class="text-sm text-muted-foreground">{{ description }}</p>
			</div>
			<div class="flex flex-col gap-2 sm:flex-row sm:items-start">
				<Popover v-if="isAdmin" v-model:open="userPickerOpen">
					<PopoverTrigger as-child>
						<ShadButton variant="outline" class="w-full justify-between font-normal sm:w-48" @click="userPickerOpen = true">
							<span :class="selectedUserId ? 'text-foreground' : 'text-muted-foreground'">
								{{ selectedUserId ? (userList.find(u => u.id === selectedUserId)?.label ?? selectedUserId) : tx('settings.analytics.all_users', 'All users') }}
							</span>
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 text-muted-foreground" />
						</ShadButton>
					</PopoverTrigger>
					<PopoverContent class="w-56 p-2" align="start">
						<ShadInput v-model="userSearch" :placeholder="tx('settings.admin_users.search', 'Search users...')" class="mb-2 h-8 text-sm" />
						<div class="max-h-52 overflow-y-auto space-y-0.5">
							<button
								type="button"
								class="w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent text-left"
								:class="!selectedUserId ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'"
								@click="selectedUserId = null; userPickerOpen = false; userSearch = ''"
							>
								<Check v-if="!selectedUserId" class="h-3.5 w-3.5 shrink-0" />
								<span v-else class="h-3.5 w-3.5 shrink-0" />
								<span>{{ tx('settings.analytics.all_users', 'All users') }}</span>
							</button>
							<button
								v-for="u in filteredUserList"
								:key="u.id"
								type="button"
								class="w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent text-left"
								:class="selectedUserId === u.id ? 'bg-accent text-accent-foreground' : 'text-foreground'"
								@click="selectedUserId = u.id; userPickerOpen = false; userSearch = ''"
							>
								<Check v-if="selectedUserId === u.id" class="h-3.5 w-3.5 shrink-0" />
								<span v-else class="h-3.5 w-3.5 shrink-0" />
								<span class="truncate">{{ u.label }}</span>
							</button>
							<p v-if="filteredUserList.length === 0" class="px-2 py-4 text-center text-xs text-muted-foreground">{{ tx('settings.admin_users.no_users', 'No users found') }}</p>
						</div>
					</PopoverContent>
				</Popover>
				<SettingsAnalyticsDateFilter @change="onDateChange" />
			</div>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-20 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<template v-else>
			<!-- Tab navigation -->
			<div class="mb-6 flex items-center gap-1 border-b border-border">
				<button
					v-for="tab in tabs"
					:key="tab.id"
					class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px"
					:class="activeTab === tab.id
						? 'text-foreground border-primary'
						: 'text-muted-foreground border-transparent hover:text-foreground hover:border-border'"
					@click="activeTab = tab.id"
				>
					{{ store.getTranslation(tab.i18n) }}
				</button>
			</div>

			<!-- Overview Tab -->
			<div v-if="activeTab === 'overview'">
				<!-- KPI summary row with sparklines -->
				<div class="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-4">
					<div class="rounded-lg border border-border bg-card p-4">
						<div class="flex items-start justify-between gap-2">
							<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.total_cost') }}</p>
							<svg viewBox="0 0 64 24" class="h-8 w-16 shrink-0" aria-hidden="true">
								<path :d="sparklineCost" fill="none" stroke-width="1.5" class="stroke-primary" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						</div>
						<p class="mt-1 text-2xl font-bold text-foreground tabular-nums">{{ formatMoney(totalCost) }}</p>
						<p v-if="costChange !== null" class="mt-1 text-xs flex items-center gap-0.5" :class="costChange <= 0 ? 'text-emerald-500' : 'text-red-400'">
							<TrendingUp v-if="costChange >= 0" class="h-3 w-3" />
							<TrendingDown v-else class="h-3 w-3" />
							{{ costChange >= 0 ? '+' : '' }}{{ costChange.toFixed(1) }}%
						</p>
					</div>
					<div class="rounded-lg border border-border bg-card p-4">
						<div class="flex items-start justify-between gap-2">
							<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.requests') }}</p>
							<svg viewBox="0 0 64 24" class="h-8 w-16 shrink-0" aria-hidden="true">
								<path :d="sparklineRequests" fill="none" stroke-width="1.5" class="stroke-primary" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						</div>
						<p class="mt-1 text-2xl font-bold text-foreground tabular-nums">{{ formatNumber(totalRequests) }}</p>
					</div>
					<div class="rounded-lg border border-border bg-card p-4">
						<div class="flex items-start justify-between gap-2">
							<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.total_tokens') }}</p>
							<svg viewBox="0 0 64 24" class="h-8 w-16 shrink-0" aria-hidden="true">
								<path :d="sparklineTokens" fill="none" stroke-width="1.5" class="stroke-primary" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						</div>
						<p class="mt-1 text-2xl font-bold text-foreground tabular-nums">{{ formatTokens(totalTokens) }}</p>
					</div>
					<div class="rounded-lg border border-border bg-card p-4">
						<div class="flex items-start justify-between gap-2">
							<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.avg_cost') }}</p>
							<svg viewBox="0 0 64 24" class="h-8 w-16 shrink-0" aria-hidden="true">
								<path :d="sparklineCost" fill="none" stroke-width="1.5" class="stroke-primary opacity-60" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						</div>
						<p class="mt-1 text-2xl font-bold text-foreground tabular-nums">{{ formatMoney(avgCostPerDay) }}</p>
						<p class="mt-1 text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.per_day') }}</p>
					</div>
				</div>

				<!-- Top API Keys + Top Users -->
				<div v-if="isAdmin" class="mb-6 grid gap-4 xl:grid-cols-2">
					<!-- Top API Keys (placeholder) -->
					<div class="rounded-lg border border-border bg-card">
						<div class="flex items-center justify-between border-b border-border px-4 py-3">
							<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_api_keys') }}</h3>
							<span class="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.coming_soon') }}</span>
						</div>
						<div class="flex flex-col items-center justify-center gap-2 py-10 text-muted-foreground">
							<KeyRound class="h-8 w-8 opacity-30" />
							<p class="text-sm">{{ store.getTranslation('settings.analytics.api_keys_soon') }}</p>
						</div>
					</div>

					<!-- Top Users -->
					<div class="rounded-lg border border-border bg-card">
						<div class="border-b border-border px-4 py-3">
							<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_users') }}</h3>
						</div>
						<div v-if="byUser.length">
							<div class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem] items-center gap-2 border-b border-border/50 px-4 py-2 text-xs font-medium text-muted-foreground">
								<span>#</span>
								<span></span>
								<span class="text-right">{{ store.getTranslation('settings.analytics.requests') }}</span>
								<span class="text-right">{{ store.getTranslation('settings.analytics.total_tokens_short') }}</span>
								<span class="text-right">{{ store.getTranslation('settings.analytics.cost') }}</span>
							</div>
							<div
								v-for="(row, i) in byUser.slice(0, 8)"
								:key="row.id ?? row.label"
								class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem] items-center gap-2 border-b border-border/50 px-4 py-2.5 text-sm last:border-0"
							>
								<span class="text-xs text-muted-foreground">{{ i + 1 }}</span>
								<div class="flex min-w-0 items-center gap-2">
									<div class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-medium uppercase text-muted-foreground">
										{{ (row.label || '?').slice(0, 1) }}
									</div>
									<span class="truncate font-medium">{{ row.label || '–' }}</span>
								</div>
								<span class="text-right text-xs text-muted-foreground">{{ formatNumber(row.request_count) }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ formatTokens(tokenTotal(row)) }}</span>
								<span class="text-right font-medium text-foreground">{{ formatMoney(Number(row.cost_total)) }}</span>
							</div>
						</div>
						<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
					</div>
				</div>

				<!-- Usage by model - stacked bar chart -->
				<div v-if="stackedBarData.length" class="mb-6 rounded-lg border border-border bg-card p-4">
					<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.usage_by_model') }}</h3>
					<SettingsStackedAnalyticsChart
						:data="stackedBarData"
						:categories="stackedBarCategories"
						:format-value="formatMoney"
					/>
				</div>

				<div class="mb-6 grid gap-4 xl:grid-cols-2">
					<!-- Token split area chart -->
					<div v-if="byDay.length" class="rounded-lg border border-border bg-card p-4">
						<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.token_split') }}</h3>
						<AreaChart
							:stacked="true"
							:data="tokenAreaData"
							:categories="tokenAreaCategories"
							:height="200"
							:x-formatter="(i: number) => tokenAreaData[i]?.day ?? ''"
							:y-formatter="(v: number) => formatTokens(v)"
							:y-grid-line="true"
							:x-num-ticks="6"
						/>
					</div>

					<!-- Request volume by model -->
					<div class="rounded-lg border border-border bg-card p-4">
						<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.request_volume') }}</h3>
						<div v-if="byModel.length">
							<BarChart
								:data="requestVolumeData"
								:categories="requestVolumeCategories"
								:height="200"
								:y-axis="requestVolumeYAxis"
								:x-formatter="(i: number) => requestVolumeData[i]?.label ?? ''"
								:y-formatter="(v: number) => formatNumber(v)"
								:hide-legend="true"
								:radius="4"
							/>
						</div>
						<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
					</div>
				</div>
			</div>

			<!-- Trends Tab -->
			<div v-if="activeTab === 'trends'">
				<div class="space-y-7">
					<section>
						<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.section_models') }}</h3>
						<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
							<div class="rounded-lg border border-border bg-card p-4">
								<div class="mb-4 flex items-center justify-between gap-3">
									<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
									<span class="text-xs font-medium text-muted-foreground tabular-nums">{{ formatMoney(totalCost) }}</span>
								</div>
								<SettingsStackedAnalyticsChart
									v-if="stackedBarData.length"
									:data="stackedBarData"
									:categories="stackedBarCategories"
									:format-value="formatMoney"
								/>
								<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
							</div>
							<div class="rounded-lg border border-border bg-card p-4">
								<div class="mb-4 flex items-center justify-between gap-3">
									<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.trending') }}</h4>
									<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.spend') }}</span>
								</div>
								<div v-if="trendingModels.length" class="space-y-4">
									<div
										v-for="model in trendingModels"
										:key="model.label"
										class="grid grid-cols-[minmax(0,1fr)_5rem_4.5rem] items-center gap-3"
									>
										<div class="flex min-w-0 items-center gap-2">
											<img v-if="model.icon?.type === 'png'" :src="model.icon.icon" class="h-5 w-5 shrink-0 rounded-md bg-muted object-cover" alt="" />
											<div
												v-else-if="model.icon?.type === 'svg'"
												class="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
												v-html="model.icon.icon"
											/>
											<span v-else class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{backgroundColor: model.color}" />
											<div class="min-w-0">
												<p class="truncate text-sm font-semibold text-foreground">{{ model.label }}</p>
												<p class="truncate text-xs text-muted-foreground">{{ formatNumber(model.requests) }} {{ store.getTranslation('settings.analytics.requests') }} | {{ formatTokens(model.tokens) }} {{ store.getTranslation('settings.analytics.tokens') }}</p>
											</div>
										</div>
										<svg viewBox="0 0 64 24" class="h-6 w-16 overflow-visible" aria-hidden="true">
											<path :d="model.sparkline" fill="none" stroke-width="1.5" :stroke="model.color" stroke-linecap="round" stroke-linejoin="round" />
										</svg>
										<div class="text-right">
											<p class="text-sm font-semibold text-foreground tabular-nums">{{ formatMoney(model.cost) }}</p>
											<p class="text-xs font-medium tabular-nums" :class="trendClass(model.pct)">{{ trendLabel(model.pct) }}</p>
										</div>
									</div>
								</div>
								<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
							</div>
						</div>
					</section>

					<section v-if="isAdmin">
						<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.section_api_keys') }}</h3>
						<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
							<div class="rounded-lg border border-border bg-card p-4">
								<div class="mb-4 flex items-center justify-between gap-3">
									<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
									<span class="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.coming_soon') }}</span>
								</div>
								<div class="flex h-[280px] flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/80 text-muted-foreground">
									<KeyRound class="h-7 w-7 opacity-40" />
									<p class="px-6 text-center text-sm">{{ store.getTranslation('settings.analytics.api_keys_soon') }}</p>
								</div>
							</div>
							<div class="rounded-lg border border-border bg-card p-4">
								<div class="mb-4 flex items-center justify-between gap-3">
									<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.trending') }}</h4>
									<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.trend') }}</span>
								</div>
								<p class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
							</div>
						</div>
					</section>

					<section v-if="isAdmin">
						<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.section_user') }}</h3>
						<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
							<div class="rounded-lg border border-border bg-card p-4">
								<div class="mb-4 flex items-center justify-between gap-3">
									<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
									<span class="text-xs font-medium text-muted-foreground tabular-nums">{{ formatMoney(userTotalCost) }}</span>
								</div>
								<SettingsHorizontalAnalyticsChart
									v-if="userBarData.length"
									:rows="userBarData"
									:value-label="store.getTranslation('settings.analytics.cost')"
									color="var(--primary)"
									:format-value="formatMoney"
								/>
								<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
							</div>
							<div class="rounded-lg border border-border bg-card p-4">
								<div class="mb-4 flex items-center justify-between gap-3">
									<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.trending') }}</h4>
									<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.spend') }}</span>
								</div>
								<div v-if="trendingUsers.length" class="space-y-4">
									<div
										v-for="user in trendingUsers"
										:key="user.id ?? user.label"
										class="grid grid-cols-[minmax(0,1fr)_5rem_4.5rem] items-center gap-3"
									>
										<div class="flex min-w-0 items-center gap-2">
											<div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-semibold uppercase text-muted-foreground">
												{{ user.initial }}
											</div>
											<div class="min-w-0">
												<p class="truncate text-sm font-semibold text-foreground">{{ user.label }}</p>
												<p class="truncate text-xs text-muted-foreground">{{ formatNumber(user.requests) }} {{ store.getTranslation('settings.analytics.requests') }} | {{ formatTokens(user.tokens) }} {{ store.getTranslation('settings.analytics.tokens') }}</p>
											</div>
										</div>
										<svg viewBox="0 0 64 24" class="h-6 w-16 overflow-visible" aria-hidden="true">
											<path :d="user.sparkline" fill="none" stroke-width="1.5" class="stroke-primary" stroke-linecap="round" stroke-linejoin="round" />
										</svg>
										<div class="text-right">
											<p class="text-sm font-semibold text-foreground tabular-nums">{{ formatMoney(user.cost) }}</p>
											<p class="text-xs font-medium text-muted-foreground tabular-nums">{{ user.share.toFixed(0) }}%</p>
										</div>
									</div>
								</div>
								<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
							</div>
						</div>
					</section>
				</div>
			</div>

			<!-- Explore Tab -->
			<div v-if="activeTab === 'explore'">
				<div class="mb-4 flex flex-wrap items-center gap-2">
					<ShadSelect v-model="exploreMetric">
						<ShadSelectTrigger size="sm" class="h-8 max-w-full min-w-0 bg-card sm:min-w-52">
							<ShadSelectValue>
								<span class="text-xs font-medium text-muted-foreground">{{ tx('settings.analytics.metric', 'Metric') }}</span>
								<span class="font-semibold text-foreground">{{ selectedExploreMetricLabel }}</span>
							</ShadSelectValue>
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="option in exploreMetricOptions" :key="option.value" :value="option.value">
								{{ option.label }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>

					<ShadSelect v-model="exploreGroup">
						<ShadSelectTrigger size="sm" class="h-8 max-w-full min-w-0 bg-card sm:min-w-44">
							<ShadSelectValue>
								<span class="text-xs font-medium text-muted-foreground">{{ tx('settings.analytics.group_by', 'Group by') }}</span>
								<span class="font-semibold text-foreground">{{ selectedExploreGroupLabel }}</span>
							</ShadSelectValue>
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="option in exploreGroupOptions" :key="option.value" :value="option.value">
								{{ option.label }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>

					<ShadSelect v-model="exploreTopN">
						<ShadSelectTrigger size="sm" class="h-8 max-w-full min-w-0 bg-card sm:min-w-24">
							<ShadSelectValue>
								<span class="text-xs font-medium text-muted-foreground">{{ tx('settings.analytics.top', 'Top') }}</span>
								<span class="font-semibold text-foreground">{{ exploreTopN }}</span>
							</ShadSelectValue>
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="option in exploreTopOptions" :key="option" :value="option">
								{{ option }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>

					<ShadSelect v-model="exploreRollup">
						<ShadSelectTrigger size="sm" class="h-8 max-w-full min-w-0 bg-card sm:min-w-36">
							<ShadSelectValue>
								<span class="text-xs font-medium text-muted-foreground">{{ tx('settings.analytics.rollup', 'Rollup') }}</span>
								<span class="font-semibold text-foreground">{{ selectedExploreRollupLabel }}</span>
							</ShadSelectValue>
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="option in exploreRollupOptions" :key="option.value" :value="option.value">
								{{ option.label }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>

				<div class="mb-6 rounded-lg border border-border bg-card px-4 py-5">
					<SettingsStackedAnalyticsChart
						v-if="exploreChartData.length"
						:data="exploreChartData"
						:categories="exploreChartCategories"
						:format-value="exploreFormatValue"
					/>
					<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>

				<div v-if="exploreTableData.length" class="rounded-lg border border-border bg-card">
					<div class="overflow-x-auto">
						<div class="min-w-[38rem]">
							<div class="grid grid-cols-[minmax(10rem,1fr)_4.75rem_4.75rem_4.75rem_4.75rem_5.25rem_6.25rem] border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
								<span>{{ tx('settings.analytics.label', 'Label') }}</span>
								<span class="text-right">{{ tx('settings.analytics.min', 'Min') }}</span>
								<span class="text-right">{{ tx('settings.analytics.max', 'Max') }}</span>
								<span class="text-right">{{ tx('settings.analytics.avg', 'Avg') }}</span>
								<span class="text-right">{{ tx('settings.analytics.sum', 'Sum') }}</span>
								<span class="text-right">{{ tx('settings.analytics.value', 'Value') }}</span>
								<span class="text-right">%</span>
							</div>
							<div
								v-for="row in exploreTableData"
								:key="row.label"
								class="grid grid-cols-[minmax(10rem,1fr)_4.75rem_4.75rem_4.75rem_4.75rem_5.25rem_6.25rem] items-center gap-1 border-b border-border/40 px-4 py-2.5 text-sm last:border-0"
							>
								<div class="flex items-center gap-2 min-w-0">
									<img v-if="row.icon?.type === 'png'" :src="row.icon.icon" class="h-5 w-5 shrink-0 rounded-md bg-muted object-cover" alt="" />
									<div
										v-else-if="row.icon?.type === 'svg'"
										class="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
										v-html="row.icon.icon"
									/>
									<span v-else class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{backgroundColor: row.color}" />
									<span class="truncate font-medium">{{ row.label }}</span>
								</div>
								<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.min) }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.max) }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.avg) }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.sum) }}</span>
								<span class="text-right font-medium text-foreground">{{ exploreFormatValue(row.value) }}</span>
								<div class="flex items-center justify-end gap-2">
									<div class="h-2 w-12 overflow-hidden rounded-full bg-muted">
										<div class="h-full rounded-full" :style="{width: `${Math.min(100, row.pct)}%`, backgroundColor: row.color}" />
									</div>
									<span class="w-9 text-right text-xs text-muted-foreground tabular-nums">{{ row.pct.toFixed(1) }}%</span>
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>

			<!-- Full model table (overview) -->
			<div v-if="activeTab === 'overview' && byModel.length" class="mt-4 rounded-lg border border-border bg-card">
				<div class="border-b border-border px-4 py-3">
					<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_models') }}</h3>
				</div>
				<div class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem_5rem_6rem] border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
					<span>#</span>
					<span>{{ store.getTranslation('settings.analytics.label') }}</span>
					<span class="text-right">{{ store.getTranslation('settings.analytics.requests') }}</span>
					<span class="text-right">Input</span>
					<span class="text-right">Output</span>
					<span class="text-right">Reasoning</span>
					<span class="text-right">{{ store.getTranslation('settings.analytics.cost') }}</span>
				</div>
				<div
					v-for="(row, i) in byModel"
					:key="row.id ?? row.label"
					class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem_5rem_6rem] items-center gap-1 border-b border-border/40 px-4 py-2.5 text-sm last:border-0"
				>
					<span class="text-xs text-muted-foreground">{{ i + 1 }}</span>
					<span class="truncate font-medium">{{ row.label || '–' }}</span>
					<span class="text-right text-xs text-muted-foreground">{{ formatNumber(row.request_count) }}</span>
					<span class="text-right text-xs text-muted-foreground">{{ row.input_tokens.toLocaleString() }}</span>
					<span class="text-right text-xs text-muted-foreground">{{ row.output_tokens.toLocaleString() }}</span>
					<span class="text-right text-xs text-muted-foreground">{{ row.reasoning_tokens.toLocaleString() }}</span>
					<span class="text-right font-medium text-foreground">{{ formatMoney(Number(row.cost_total)) }}</span>
				</div>
			</div>
		</template>
	</div>
</template>

<script setup lang="ts">
import {Loader2, TrendingUp, TrendingDown, KeyRound, ChevronsUpDown, Check} from 'lucide-vue-next';
import {AreaChart, BarChart} from 'vue-chrts';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';
import {useMainStore} from '@/stores';
import {useBudgetStore} from '@/stores/budgetStore';
import {useIconsStore} from '@/stores/icons';
import type {AnalyticsDayModelRow, AnalyticsRow} from '~/types/budgets';

const props = defineProps<{
	isAdmin: boolean;
	userId?: string;
}>();

interface UserEntry {
	id: string;
	label: string;
}

const store = useMainStore();
const budgetStore = useBudgetStore();
const iconStore = useIconsStore();
const {$customFetch} = useNuxtApp();
const loading = ref(false);
const from = ref('');
const to = ref('');
const selectedUserId = ref<string | null>(props.userId ?? null);
const userList = ref<UserEntry[]>([]);
const userPickerOpen = ref(false);
const userSearch = ref('');

const filteredUserList = computed(() => {
	const q = userSearch.value.trim().toLowerCase();
	return q ? userList.value.filter(u => u.label.toLowerCase().includes(q)) : userList.value;
});
const activeTab = ref('overview');
const exploreMetric = ref('cost');
const exploreGroup = ref('model');
const exploreTopN = ref(10);
const exploreRollup = ref('hourly');

const exploreTopOptions = [5, 10, 20];

function tx(key: string, fallback: string): string {
	const value = store.getTranslation(key);
	return value === key ? fallback : value;
}

const exploreMetricOptions = computed(() => [
	{value: 'requests', label: tx('settings.analytics.request_count', 'Request count')},
	{value: 'cost', label: tx('settings.analytics.total_usage_dollars', 'Total usage ($)')},
	{value: 'tokens_total', label: tx('settings.analytics.tokens_total', 'Tokens (total)')},
	{value: 'tokens_input', label: tx('settings.analytics.tokens_prompt', 'Tokens (prompt)')},
	{value: 'tokens_output', label: tx('settings.analytics.tokens_completion', 'Tokens (completion)')},
	{value: 'tokens_reasoning', label: tx('settings.analytics.reasoning_tokens', 'Reasoning tokens')},
]);

const exploreGroupOptions = computed(() => [
	{value: 'model', label: tx('settings.analytics.model', 'Model')},
	{value: 'user', label: tx('settings.analytics.user', 'User')},
]);

const exploreRollupOptions = computed(() => [
	{value: 'hourly', label: tx('settings.analytics.hourly', 'Hourly')},
	{value: 'daily', label: tx('settings.analytics.daily', 'Daily')},
	{value: 'weekly', label: tx('settings.analytics.weekly', 'Weekly')},
]);

const selectedExploreMetricLabel = computed(() => exploreMetricOptions.value.find(option => option.value === exploreMetric.value)?.label ?? '');
const selectedExploreGroupLabel = computed(() => exploreGroupOptions.value.find(option => option.value === exploreGroup.value)?.label ?? '');
const selectedExploreRollupLabel = computed(() => exploreRollupOptions.value.find(option => option.value === exploreRollup.value)?.label ?? '');

const CHART_COLORS = [
	'#ef4444', '#f97316', '#eab308', '#22c55e', '#06b6d4',
	'#3b82f6', '#8b5cf6', '#ec4899', '#f43f5e', '#14b8a6',
	'#a855f7', '#6366f1', '#d946ef', '#0ea5e9', '#84cc16',
];

const title = computed(() => {
	if (props.isAdmin && selectedUserId.value) {
		const user = userList.value.find(u => u.id === selectedUserId.value);
		return user?.label ?? store.getTranslation('settings.tabs.analytics');
	}
	return props.isAdmin
		? store.getTranslation('settings.tabs.analytics')
		: store.getTranslation('settings.analytics.my_usage');
});

const description = computed(() => store.getTranslation('settings.analytics.description'));

const tabs = [
	{id: 'overview', i18n: 'settings.analytics.tab_overview'},
	{id: 'trends', i18n: 'settings.analytics.tab_trends'},
	{id: 'explore', i18n: 'settings.analytics.tab_explore'},
];

function onDateChange(range: {from: string; to: string; label: string}) {
	from.value = range.from;
	to.value = range.to;
	load();
}

const byModel = computed(() => {
	if (props.isAdmin && selectedUserId.value) return budgetStore.userAnalytics.byModel;
	if (props.isAdmin) return budgetStore.analytics.byModel;
	return budgetStore.myAnalytics.byModel;
});
const byDay = computed(() => {
	if (props.isAdmin && selectedUserId.value) return budgetStore.userAnalytics.byDay;
	if (props.isAdmin) return budgetStore.analytics.byDay;
	return budgetStore.myAnalytics.byDay;
});
const byDayModel = computed(() => {
	if (props.isAdmin && selectedUserId.value) return budgetStore.userAnalytics.byDayModel;
	if (props.isAdmin) return budgetStore.analytics.byDayModel;
	return budgetStore.myAnalytics.byDayModel;
});
const byUser = computed(() => (props.isAdmin && !selectedUserId.value) ? budgetStore.analytics.byUser : []);
const byTeam = computed(() => (props.isAdmin && !selectedUserId.value) ? budgetStore.analytics.byTeam : []);

function tokenTotal(row: AnalyticsRow) {
	return row.input_tokens + row.output_tokens + row.reasoning_tokens;
}

const totalCost = computed(() => byModel.value.reduce((s, r) => s + Number(r.cost_total), 0));
const totalTokens = computed(() => byModel.value.reduce((s, r) => s + tokenTotal(r), 0));
const totalRequests = computed(() => byModel.value.reduce((s, r) => s + r.request_count, 0));

const dayCount = computed(() => {
	if (!from.value || !to.value) return 30;
	const d = (new Date(to.value).getTime() - new Date(from.value).getTime()) / 86_400_000;
	return Math.max(1, Math.round(d));
});

const avgCostPerDay = computed(() => (dayCount.value > 0 ? totalCost.value / dayCount.value : 0));

const costChange = computed(() => {
	const days = byDay.value;
	if (days.length < 2) return null;
	const mid = Math.floor(days.length / 2);
	const firstHalf = days.slice(0, mid).reduce((s, r) => s + Number(r.cost_total), 0);
	const secondHalf = days.slice(mid).reduce((s, r) => s + Number(r.cost_total), 0);
	if (firstHalf === 0) return null;
	return ((secondHalf - firstHalf) / firstHalf) * 100;
});

function computeSparklinePath(values: number[]): string {
	if (values.length < 2) return '';
	const min = Math.min(...values);
	const max = Math.max(...values);
	const range = max - min || 1;
	const w = 64;
	const h = 24;
	const padding = 2;
	const points = values.map((v, i) => {
		const x = padding + (i / (values.length - 1)) * (w - padding * 2);
		const y = h - padding - ((v - min) / range) * (h - padding * 2);
		return `${x.toFixed(1)},${y.toFixed(1)}`;
	});
	return `M${points.join('L')}`;
}

const sparklineCost = computed(() => computeSparklinePath(byDay.value.map(r => Number(r.cost_total))));
const sparklineRequests = computed(() => computeSparklinePath(byDay.value.map(r => r.request_count)));
const sparklineTokens = computed(() => computeSparklinePath(byDay.value.map(r => r.input_tokens + r.output_tokens + r.reasoning_tokens)));

const topModelNames = computed(() => byModel.value.slice(0, 8).map(r => r.label));

const modelColorMap = computed(() => {
	const map: Record<string, string> = {};
	topModelNames.value.forEach((name, i) => {
		map[name] = CHART_COLORS[i % CHART_COLORS.length];
	});
	map['Other'] = '#6b7280';
	return map;
});

const stackedBarData = computed(() => {
	const rows = byDayModel.value;
	if (!rows.length) return [];
	const topNames = new Set(topModelNames.value);
	const dayMap = new Map<string, Record<string, number>>();
	for (const row of rows) {
		const bucket = topNames.has(row.model_name) ? row.model_name : 'Other';
		if (!dayMap.has(row.day)) dayMap.set(row.day, {day: 0} as any);
		const entry = dayMap.get(row.day)!;
		(entry as any).day = row.day;
		entry[bucket] = (entry[bucket] || 0) + Number(row.cost_total);
	}
	return Array.from(dayMap.values());
});

const stackedBarCategories = computed(() => {
	const cats: Record<string, {name: string; color: string}> = {};
	for (const name of topModelNames.value) {
		cats[name] = {name, color: modelColorMap.value[name]};
	}
	if (byDayModel.value.some(r => !topModelNames.value.includes(r.model_name))) {
		cats['Other'] = {name: 'Other', color: '#6b7280'};
	}
	return cats;
});

const tokenAreaData = computed(() =>
	byDay.value.map(r => ({
		day: r.label,
		input: r.input_tokens,
		output: r.output_tokens,
		reasoning: r.reasoning_tokens,
	})),
);

const tokenAreaCategories = {
	input: {name: 'Input', color: '#3b82f6'},
	output: {name: 'Output', color: '#8b5cf6'},
	reasoning: {name: 'Reasoning', color: '#06b6d4'},
};

const requestVolumeData = computed(() =>
	byModel.value.slice(0, 10).map(r => ({label: r.label, requests: r.request_count})),
);

const requestVolumeCategories = {
	requests: {name: 'Requests', color: 'var(--primary)'},
};

const requestVolumeYAxis = ['requests'];

const userBarData = computed(() =>
	byUser.value.slice(0, 8).map(row => ({label: row.label || '-', value: Number(row.cost_total)})),
);

const userTotalCost = computed(() => byUser.value.reduce((sum, row) => sum + Number(row.cost_total), 0));

const trendingModels = computed(() => {
	const days = [...new Set(byDayModel.value.map(row => row.day))].sort();
	const mid = Math.floor(days.length / 2);
	const firstDays = new Set(days.slice(0, mid));
	const secondDays = new Set(days.slice(mid));
	const spendByModel = new Map<string, {first: number; second: number}>();
	const seriesByModel = new Map<string, number[]>();
	for (const row of byDayModel.value) {
		const spend = spendByModel.get(row.model_name) ?? {first: 0, second: 0};
		if (firstDays.has(row.day)) {
			spend.first += Number(row.cost_total);
		} else if (secondDays.has(row.day)) {
			spend.second += Number(row.cost_total);
		}
		spendByModel.set(row.model_name, spend);
		const series = seriesByModel.get(row.model_name) ?? Array.from({length: days.length}, () => 0);
		const dayIndex = days.indexOf(row.day);
		if (dayIndex >= 0) {
			series[dayIndex] += Number(row.cost_total);
		}
		seriesByModel.set(row.model_name, series);
	}
	return byModel.value.slice(0, 8).map((model, i) => ({
		id: model.id,
		label: model.label,
		color: CHART_COLORS[i % CHART_COLORS.length],
		pct: trendPercent(spendByModel.get(model.label)),
		cost: Number(model.cost_total),
		requests: model.request_count,
		tokens: tokenTotal(model),
		sparkline: computeSparklinePath(seriesByModel.get(model.label) ?? []),
		icon: getModelIcon(model.label, model.id),
	}));
});

const trendingUsers = computed(() => {
	const total = userTotalCost.value;
	return byUser.value.slice(0, 8).map((row, index) => {
		const cost = Number(row.cost_total);
		return {
			id: row.id,
			label: row.label || '-',
			initial: (row.label || '?').slice(0, 1),
			cost,
			requests: row.request_count,
			tokens: tokenTotal(row),
			share: total > 0 ? (cost / total) * 100 : 0,
			sparkline: computeSparklinePath(rankSparklineValues(cost, index)),
		};
	});
});

function rankSparklineValues(value: number, index: number): number[] {
	const base = Math.max(value, 0.0001);
	const offset = (index % 4) * 0.06;
	return [
		base * (0.52 + offset),
		base * (0.64 + offset),
		base * (0.58 + offset),
		base * (0.76 + offset),
		base * (0.7 + offset),
		base,
	];
}

function trendPercent(spend: {first: number; second: number} | undefined): number {
	if (!spend || spend.first === 0) return 0;
	return ((spend.second - spend.first) / spend.first) * 100;
}

function trendClass(value: number): string {
	if (value < 0) return 'text-red-400';
	if (value > 0) return 'text-emerald-500';
	return 'text-muted-foreground';
}

function trendLabel(value: number): string {
	if (value === 0) return '0%';
	return `${value > 0 ? '↑' : '↓'} ${Math.abs(value).toFixed(0)}%`;
}

function getModelIcon(label: string, id: string | null) {
	return iconStore.getProviderIcon(label, id ?? undefined);
}

function getExploreIcon(label: string) {
	if (exploreGroup.value !== 'model') return null;
	const model = byModel.value.find(row => row.label === label);
	return getModelIcon(label, model?.id ?? null);
}

type ExploreMetricRow = {
	cost_total: string;
	request_count: number;
	input_tokens: number;
	output_tokens: number;
	reasoning_tokens: number;
};

function getExploreMetricValue(row: ExploreMetricRow): number {
	switch (exploreMetric.value) {
		case 'cost': return Number(row.cost_total);
		case 'requests': return row.request_count;
		case 'tokens_total': return row.input_tokens + row.output_tokens + row.reasoning_tokens;
		case 'tokens_input': return row.input_tokens;
		case 'tokens_output': return row.output_tokens;
		case 'tokens_reasoning': return row.reasoning_tokens;
		case 'latency': return 0;
		default: return Number(row.cost_total);
	}
}

function getMetricValue(row: AnalyticsDayModelRow): number {
	return getExploreMetricValue(row);
}

function getSummaryMetricValue(row: AnalyticsRow): number {
	return getExploreMetricValue(row);
}

function exploreBucket(day: string): string {
	if (exploreRollup.value !== 'weekly') return day;
	const date = new Date(`${day}T00:00:00Z`);
	if (Number.isNaN(date.getTime())) return day;
	const weekStart = new Date(date);
	const dayOfWeek = weekStart.getUTCDay() || 7;
	weekStart.setUTCDate(weekStart.getUTCDate() - dayOfWeek + 1);
	return weekStart.toISOString().slice(0, 10);
}

const exploreAggregateRows = computed(() => {
	if (exploreGroup.value === 'user') return byUser.value;
	return [] as AnalyticsRow[];
});

const exploreChartData = computed(() => {
	if (exploreGroup.value === 'user') {
		const rows = exploreTableData.value;
		if (!rows.length) return [];
		return [
			rows.reduce((entry, row) => {
				entry[row.label] = row.value;
				return entry;
			}, {day: store.getTranslation('settings.analytics.total')} as Record<string, number | string>),
		];
	}

	if (exploreGroup.value !== 'model') return [];

	const rows = byDayModel.value;
	if (!rows.length) return [];

	const modelTotals = new Map<string, number>();
	for (const row of rows) {
		modelTotals.set(row.model_name, (modelTotals.get(row.model_name) || 0) + getMetricValue(row));
	}
	const topNames = [...modelTotals.entries()]
		.sort((a, b) => b[1] - a[1])
		.slice(0, exploreTopN.value)
		.map(([name]) => name);
	const topSet = new Set(topNames);

	const dayMap = new Map<string, Record<string, number>>();
	for (const row of rows) {
		const bucket = topSet.has(row.model_name) ? row.model_name : 'Other';
		const day = exploreBucket(row.day);
		if (!dayMap.has(day)) dayMap.set(day, {} as any);
		const entry = dayMap.get(day)!;
		(entry as any).day = day;
		entry[bucket] = (entry[bucket] || 0) + getMetricValue(row);
	}
	return Array.from(dayMap.values());
});

const exploreChartCategories = computed(() => {
	if (exploreGroup.value === 'user') {
		const cats: Record<string, {name: string; color: string}> = {};
		exploreTableData.value.forEach((row, i) => {
			cats[row.label] = {name: row.label, color: CHART_COLORS[i % CHART_COLORS.length]};
		});
		return cats;
	}

	if (exploreGroup.value !== 'model') return {};

	const modelTotals = new Map<string, number>();
	for (const row of byDayModel.value) {
		modelTotals.set(row.model_name, (modelTotals.get(row.model_name) || 0) + getMetricValue(row));
	}
	const topNames = [...modelTotals.entries()]
		.sort((a, b) => b[1] - a[1])
		.slice(0, exploreTopN.value)
		.map(([name]) => name);

	const cats: Record<string, {name: string; color: string}> = {};
	topNames.forEach((name, i) => {
		cats[name] = {name, color: CHART_COLORS[i % CHART_COLORS.length]};
	});
	if (byDayModel.value.some(r => !topNames.includes(r.model_name))) {
		cats['Other'] = {name: store.getTranslation('settings.analytics.other'), color: '#6b7280'};
	}
	return cats;
});

const exploreTableData = computed(() => {
	if (exploreGroup.value !== 'model') {
		const rows = exploreAggregateRows.value
			.map(row => ({
				label: row.label || '-',
				value: getSummaryMetricValue(row),
			}))
			.sort((a, b) => b.value - a.value);
		const total = rows.reduce((sum, row) => sum + row.value, 0);
		return rows.slice(0, exploreTopN.value).map((row, i) => ({
			label: row.label,
			color: CHART_COLORS[i % CHART_COLORS.length],
			icon: null,
			min: row.value,
			max: row.value,
			avg: row.value,
			sum: row.value,
			value: row.value,
			pct: total > 0 ? (row.value / total) * 100 : 0,
		}));
	}

	const modelDailyValues = new Map<string, number[]>();
	for (const row of byDayModel.value) {
		const vals = modelDailyValues.get(row.model_name) || [];
		vals.push(getMetricValue(row));
		modelDailyValues.set(row.model_name, vals);
	}

	const totalAll = [...modelDailyValues.values()].reduce((s, vals) => s + vals.reduce((a, b) => a + b, 0), 0);

	const modelTotals = [...modelDailyValues.entries()].map(([name, vals]) => ({
		label: name,
		sum: vals.reduce((a, b) => a + b, 0),
		vals,
	}));
	modelTotals.sort((a, b) => b.sum - a.sum);

	return modelTotals.slice(0, exploreTopN.value).map((m, i) => ({
		label: m.label,
		color: CHART_COLORS[i % CHART_COLORS.length],
		icon: getExploreIcon(m.label),
		min: Math.min(...m.vals),
		max: Math.max(...m.vals),
		avg: m.sum / Math.max(1, m.vals.length),
		sum: m.sum,
		value: m.sum,
		pct: totalAll > 0 ? (m.sum / totalAll) * 100 : 0,
	}));
});

function exploreFormatValue(v: number): string {
	if (exploreMetric.value === 'cost') return formatMoney(v);
	if (exploreMetric.value === 'latency') return `${v.toFixed(0)} ms`;
	return formatNumber(v);
}

function formatMoney(value: number) {
	if (value >= 1) return `$${value.toFixed(2)}`;
	return `$${value.toFixed(4)}`;
}

function formatTokens(n: number) {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
	return String(n);
}

function formatNumber(n: number) {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
	return n.toLocaleString();
}

async function loadUsers() {
	const res = await $customFetch<{users: {id: string; username: string; email: string}[]}>('/api/v1/admin/users', {params: {per_page: 200}});
	userList.value = (res?.users ?? []).map(u => ({id: u.id, label: u.username || u.email}));
}

async function load() {
	loading.value = true;
	try {
		const params = {
			from: from.value ? `${from.value}T00:00:00Z` : undefined,
			to: to.value ? `${to.value}T23:59:59Z` : undefined,
		};
		if (props.isAdmin && selectedUserId.value) {
			await budgetStore.fetchUserAnalytics(selectedUserId.value, params);
		} else if (props.isAdmin) {
			await budgetStore.fetchAllAnalytics(params);
		} else {
			await budgetStore.fetchMyAnalytics(params);
		}
	} catch {
		store.toast(store.getTranslation('common.error'), {type: 'error'});
	} finally {
		loading.value = false;
	}
}

watch(() => props.userId, (v) => {
	selectedUserId.value = v ?? null;
});

watch(selectedUserId, () => {
	load();
});

onMounted(() => {
	if (props.isAdmin) loadUsers();
});
</script>
