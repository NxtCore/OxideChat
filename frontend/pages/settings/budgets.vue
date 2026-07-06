<template>
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="mb-6 flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.budgets') }}</h2>
				<p class="text-sm text-muted-foreground">{{ tx('settings.budgets.description', 'Limit spend for users and teams') }}</p>
			</div>
			<div class="flex items-center gap-2">
				<ShadButton variant="outline" size="sm" class="gap-2" @click="loadOperationalData">
					<RefreshCcw class="h-4 w-4" />
					<span>{{ tx('common.refresh', 'Refresh') }}</span>
				</ShadButton>
				<ShadButton v-if="canEdit" variant="default" size="sm" class="gap-2" @click="activeTab = 'budgets'; createBudget()">
					<Plus class="h-4 w-4" />
					<span>{{ tx('settings.budgets.create', 'Create budget') }}</span>
				</ShadButton>
			</div>
		</div>

		<ShadTabs v-model="activeTab" class="w-full">
			<ShadTabsList class="mb-4 grid w-full grid-cols-4 md:w-[34rem]">
				<ShadTabsTrigger value="users">{{ tx('settings.budgets.tab_users', 'Users') }}</ShadTabsTrigger>
				<ShadTabsTrigger value="teams">{{ tx('settings.budgets.tab_teams', 'Teams') }}</ShadTabsTrigger>
				<ShadTabsTrigger value="budgets">{{ tx('settings.budgets.tab_budgets', 'Budgets') }}</ShadTabsTrigger>
				<ShadTabsTrigger value="history">{{ tx('settings.budgets.tab_history', 'History') }}</ShadTabsTrigger>
			</ShadTabsList>

			<ShadTabsContent value="users" class="mt-0 space-y-4">
				<div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
					<ShadInput v-model="userOverviewSearch" class="md:max-w-sm" :placeholder="tx('settings.budgets.search_users', 'Search users')" />
					<div class="flex items-center gap-2 text-sm text-muted-foreground">
						<span>{{ filteredUserOverview.length }}</span>
						<span>{{ tx('settings.budgets.users_with_budgets', 'users') }}</span>
					</div>
				</div>

				<div v-if="loading" class="flex items-center justify-center py-10 text-muted-foreground">
					<Loader2 class="h-5 w-5 animate-spin" />
				</div>
				<div v-else class="overflow-hidden rounded-lg border border-border bg-card">
					<div class="grid grid-cols-[minmax(12rem,1.25fr)_minmax(9rem,1fr)_minmax(12rem,1.25fr)_7rem_7rem_9rem] gap-3 border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
						<span>{{ tx('settings.budgets.user', 'User') }}</span>
						<span>{{ tx('settings.budgets.teams', 'Teams') }}</span>
						<span>{{ tx('settings.budgets.effective_budgets', 'Effective budgets') }}</span>
						<span>{{ tx('settings.budgets.used', 'Used') }}</span>
						<span>{{ tx('settings.budgets.remaining', 'Remaining') }}</span>
						<span>{{ tx('settings.budgets.actions', 'Actions') }}</span>
					</div>
					<div v-if="filteredUserOverview.length === 0" class="p-6 text-center text-sm text-muted-foreground">
						{{ tx('settings.budgets.no_user_budgets', 'No user budgets found') }}
					</div>
					<div v-for="row in filteredUserOverview" v-else :key="row.user_id" class="border-b border-border last:border-b-0">
						<div class="grid grid-cols-[minmax(12rem,1.25fr)_minmax(9rem,1fr)_minmax(12rem,1.25fr)_7rem_7rem_9rem] items-center gap-3 px-4 py-3 text-sm">
							<div class="min-w-0">
								<div class="truncate font-medium text-foreground">{{ row.user_label }}</div>
								<div class="text-xs text-muted-foreground">{{ row.budgets.length }} {{ tx('settings.budgets.budget_count', 'budgets') }}</div>
							</div>
							<div class="flex min-w-0 flex-wrap gap-1">
								<span v-for="team in row.teams.slice(0, 2)" :key="team.id" class="rounded-md bg-muted px-2 py-0.5 text-xs text-muted-foreground">{{ team.name }}</span>
								<span v-if="row.teams.length > 2" class="rounded-md bg-muted px-2 py-0.5 text-xs text-muted-foreground">+{{ row.teams.length - 2 }}</span>
							</div>
							<div class="min-w-0 space-y-1">
								<div v-for="budget in row.budgets.slice(0, 2)" :key="budget.assignment_id" class="flex items-center gap-2">
									<span class="truncate">{{ budget.budget.name }}</span>
									<span class="shrink-0 rounded-md bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">{{ budget.budget.kind }}</span>
								</div>
								<div v-if="row.budgets.length > 2" class="text-xs text-muted-foreground">+{{ row.budgets.length - 2 }}</div>
							</div>
							<span>{{ formatMoney(row.spent) }}</span>
							<span :class="Number(row.remaining) <= 0 ? 'text-destructive' : 'text-foreground'">{{ formatMoney(row.remaining) }}</span>
							<div class="flex items-center gap-1">
								<ShadButton v-if="canEdit" variant="outline" size="sm" class="gap-2" @click="resetUserRow(row)">
									<RotateCcw class="h-4 w-4" />
									<span>{{ tx('settings.budgets.reset', 'Reset') }}</span>
								</ShadButton>
								<ShadButton variant="ghost" size="icon" class="h-8 w-8" @click="toggleUser(row.user_id)">
									<ChevronDown class="h-4 w-4 transition-transform" :class="expandedUsers.has(row.user_id) ? 'rotate-180' : ''" />
								</ShadButton>
							</div>
						</div>
						<div v-if="expandedUsers.has(row.user_id)" class="border-t border-border bg-muted/20 px-4 py-3">
							<div class="grid gap-2">
								<div v-for="budget in row.budgets" :key="budget.assignment_id" class="grid gap-2 rounded-md border border-border bg-background px-3 py-2 text-sm md:grid-cols-[1.4fr_7rem_7rem_7rem_9rem_4rem] md:items-center">
									<div class="min-w-0">
										<div class="truncate font-medium">{{ budget.budget.name }}</div>
										<div class="text-xs text-muted-foreground">{{ budget.budget.kind }} · {{ budget.budget.interval }} · {{ sourceLabel(budget) }}</div>
									</div>
									<span>{{ formatMoney(budget.spent) }}</span>
									<span>{{ formatMoney(budget.amount) }}</span>
									<span>{{ formatMoney(budget.remaining) }}</span>
									<span class="text-xs text-muted-foreground">{{ budget.resets_at ? formatDate(budget.resets_at) : tx('settings.budgets.no_reset', 'No reset') }}</span>
									<ShadButton v-if="canEdit" variant="outline" size="sm" class="gap-2" @click="openReset({assignment_id: budget.assignment_id, user_id: row.user_id}, `${row.user_label} · ${budget.budget.name}`)">
										<RotateCcw class="h-3.5 w-3.5" />
									</ShadButton>
								</div>
							</div>
						</div>
					</div>
				</div>
			</ShadTabsContent>

			<ShadTabsContent value="teams" class="mt-0 space-y-4">
				<ShadInput v-model="teamOverviewSearch" class="md:max-w-sm" :placeholder="tx('settings.budgets.search_teams', 'Search teams')" />
				<div v-if="loading" class="flex items-center justify-center py-10 text-muted-foreground">
					<Loader2 class="h-5 w-5 animate-spin" />
				</div>
				<div v-else class="grid gap-3">
					<div v-for="team in filteredTeamOverview" :key="team.team_id" class="rounded-lg border border-border bg-card p-4">
						<div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
							<div>
								<h3 class="font-semibold text-foreground">{{ team.team_name }}</h3>
								<p class="text-sm text-muted-foreground">{{ team.member_count }} {{ tx('settings.budgets.members', 'members') }} · {{ team.budgets.length }} {{ tx('settings.budgets.budget_count', 'budgets') }}</p>
							</div>
							<div class="flex flex-wrap items-center gap-2 text-sm">
								<span>{{ formatMoney(team.spent) }} {{ tx('settings.budgets.used', 'Used') }}</span>
								<span class="text-muted-foreground">·</span>
								<span>{{ formatMoney(team.remaining) }} {{ tx('settings.budgets.remaining', 'Remaining') }}</span>
								<ShadButton v-if="canEdit" variant="outline" size="sm" class="gap-2" @click="openReset({team_id: team.team_id}, team.team_name)">
									<RotateCcw class="h-4 w-4" />
									<span>{{ tx('settings.budgets.reset', 'Reset') }}</span>
								</ShadButton>
							</div>
						</div>
						<div class="mt-3 grid gap-2">
							<div v-for="budget in team.budgets" :key="budget.assignment_id" class="grid gap-2 rounded-md border border-border px-3 py-2 text-sm md:grid-cols-[1.4fr_7rem_7rem_8rem_6rem] md:items-center">
								<div>
									<div class="font-medium">{{ budget.budget.name }}</div>
									<div class="text-xs text-muted-foreground">{{ budget.budget.kind }} · {{ budget.budget.interval }} · {{ budget.affected_users }} {{ tx('settings.budgets.affected_users', 'affected') }}</div>
								</div>
								<span>{{ formatMoney(budget.spent) }}</span>
								<span>{{ formatMoney(budget.remaining) }}</span>
								<span class="text-xs text-muted-foreground">{{ budget.resets_at ? formatDate(budget.resets_at) : tx('settings.budgets.no_reset', 'No reset') }}</span>
								<ShadButton v-if="canEdit" variant="ghost" size="sm" class="gap-2" @click="openReset({assignment_id: budget.assignment_id}, budget.budget.name)">
									<RotateCcw class="h-3.5 w-3.5" />
								</ShadButton>
							</div>
						</div>
					</div>
					<div v-if="filteredTeamOverview.length === 0" class="rounded-lg border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
						{{ tx('settings.budgets.no_team_budgets', 'No team budgets found') }}
					</div>
				</div>
			</ShadTabsContent>

			<ShadTabsContent value="budgets" class="mt-0">
				<div class="grid gap-4 lg:grid-cols-[19rem_1fr]">
					<div class="space-y-3">
						<div v-if="loading" class="flex items-center justify-center py-10 text-muted-foreground">
							<Loader2 class="h-5 w-5 animate-spin" />
						</div>
						<div v-else-if="budgetStore.budgets.length === 0" class="rounded-lg border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
							{{ tx('settings.budgets.create', 'Create budget') }}
						</div>
						<div v-else class="space-y-2">
							<button
								v-for="budget in budgetStore.budgets"
								:key="budget.id"
								type="button"
								class="w-full rounded-lg border px-3 py-2.5 text-left transition-colors"
								:class="selected?.id === budget.id ? 'border-primary/60 bg-accent/40' : 'border-border bg-card hover:border-primary/40 hover:bg-accent/10'"
								@click="selectBudget(budget)"
							>
								<div class="flex items-center justify-between gap-2">
									<span class="truncate font-medium text-foreground">{{ budget.name }}</span>
									<span class="shrink-0 rounded-full px-2 py-0.5 text-xs font-medium" :class="decisionClass(budget.on_exceed)">
										{{ budget.on_exceed }}
									</span>
								</div>
								<div class="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
									<span class="font-medium text-foreground">{{ formatMoney(budget.amount) }}</span>
									<span>·</span>
									<span>{{ budget.kind }}</span>
									<span>·</span>
									<span>{{ budget.interval }}</span>
								</div>
							</button>
						</div>
					</div>

					<div v-if="selected" class="space-y-4">
						<div class="rounded-lg border border-border bg-card p-4">
							<div class="mb-4 flex items-center justify-between gap-2">
								<h3 class="text-sm font-semibold text-foreground">{{ tx('settings.budgets.settings', 'Settings') }}</h3>
								<div v-if="canEdit" class="flex items-center gap-2">
									<ShadButton variant="outline" size="sm" class="text-destructive hover:text-destructive" @click="deleteBudget">
										<Trash2 class="h-4 w-4" />
									</ShadButton>
									<ShadButton variant="default" size="sm" @click="saveBudget">{{ tx('common.save', 'Save') }}</ShadButton>
								</div>
							</div>
							<div class="grid gap-4 md:grid-cols-2">
								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.name', 'Name') }}</ShadLabel>
									<ShadInput v-model="form.name" :disabled="!canEdit" />
								</div>
								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.amount', 'Amount') }}</ShadLabel>
									<ShadInput v-model="form.amount" :disabled="!canEdit" type="number" step="0.0001" />
								</div>
								<div class="space-y-1.5 md:col-span-2">
									<ShadLabel>{{ tx('settings.budgets.notes', 'Description') }}</ShadLabel>
									<ShadTextarea :model-value="form.description ?? ''" :disabled="!canEdit" rows="2" @update:model-value="form.description = $event || null" />
								</div>
								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.kind', 'Kind') }}</ShadLabel>
									<ShadSelect v-model="form.kind" :disabled="!canEdit">
										<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
										<ShadSelectContent>
											<ShadSelectItem value="pooled">pooled</ShadSelectItem>
											<ShadSelectItem value="per_user">per_user</ShadSelectItem>
										</ShadSelectContent>
									</ShadSelect>
								</div>
								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.interval', 'Interval') }}</ShadLabel>
									<ShadSelect v-model="form.interval" :disabled="!canEdit">
										<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
										<ShadSelectContent>
											<ShadSelectItem value="daily">daily</ShadSelectItem>
											<ShadSelectItem value="weekly">weekly</ShadSelectItem>
											<ShadSelectItem value="monthly">monthly</ShadSelectItem>
											<ShadSelectItem value="total">total</ShadSelectItem>
										</ShadSelectContent>
									</ShadSelect>
								</div>
								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.reset_strategy', 'Reset') }}</ShadLabel>
									<ShadSelect v-model="form.reset_strategy" :disabled="!canEdit">
										<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
										<ShadSelectContent>
											<ShadSelectItem value="calendar">calendar</ShadSelectItem>
											<ShadSelectItem value="rolling">rolling</ShadSelectItem>
											<ShadSelectItem value="anchored">anchored</ShadSelectItem>
										</ShadSelectContent>
									</ShadSelect>
								</div>
								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.on_exceed', 'Action') }}</ShadLabel>
									<ShadSelect v-model="form.on_exceed" :disabled="!canEdit">
										<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
										<ShadSelectContent>
											<ShadSelectItem value="block">block</ShadSelectItem>
											<ShadSelectItem value="warn">warn</ShadSelectItem>
											<ShadSelectItem value="allow">allow</ShadSelectItem>
										</ShadSelectContent>
									</ShadSelect>
								</div>
							</div>
							<div class="mt-4 flex items-center gap-2 border-t border-border pt-4">
								<ShadCheckbox
									:model-value="form.is_enabled"
									:disabled="!canEdit"
									@update:model-value="checked => form.is_enabled = Boolean(checked)"
								/>
								<ShadLabel>{{ tx('settings.budgets.enabled', 'Enabled') }}</ShadLabel>
							</div>
						</div>

						<div class="rounded-lg border border-border bg-card p-4">
							<h3 class="mb-3 text-sm font-semibold text-foreground">{{ tx('settings.budgets.assignments', 'Assignments') }}</h3>
							<div v-if="budgetStore.assignments.length" class="mb-3 space-y-2">
								<div v-for="a in budgetStore.assignments" :key="a.id" class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2 text-sm">
									<div class="flex min-w-0 items-center gap-2">
										<component :is="a.team_id ? Building2 : User" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
										<span class="truncate font-medium">{{ a.team_name ?? a.user_label ?? '-' }}</span>
										<span class="shrink-0 text-xs text-muted-foreground">{{ a.team_id ? tx('settings.budgets.team', 'team') : tx('settings.budgets.user', 'user') }}</span>
									</div>
									<div class="flex items-center gap-1">
										<ShadButton v-if="canEdit" variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-destructive" @click="openReset({assignment_id: a.id}, a.team_name ?? a.user_label ?? selected.name)">
											<RotateCcw class="h-4 w-4" />
										</ShadButton>
										<ShadButton v-if="canEdit" variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-destructive" @click="removeAssignment(a)">
											<X class="h-4 w-4" />
										</ShadButton>
									</div>
								</div>
							</div>
							<p v-else class="mb-3 text-sm text-muted-foreground">{{ tx('settings.budgets.no_assignments', 'No assignments yet') }}</p>

							<div v-if="canEdit" class="grid gap-3 md:grid-cols-2">
								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.assign_team', 'Assign team') }}</ShadLabel>
									<Popover v-model:open="teamPickerOpen">
										<PopoverTrigger as-child>
											<ShadButton variant="outline" class="w-full justify-between font-normal" @click="teamPickerOpen = true">
												<span class="truncate" :class="selectedTeamId ? 'text-foreground' : 'text-muted-foreground'">
													{{ selectedTeamId ? (teams.find(t => t.id === selectedTeamId)?.name ?? selectedTeamId) : tx('settings.budgets.pick_team', 'Select a team') }}
												</span>
												<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 text-muted-foreground" />
											</ShadButton>
										</PopoverTrigger>
										<PopoverContent class="w-72 p-2" align="start">
											<ShadInput v-model="teamSearch" :placeholder="tx('settings.teams.search', 'Search teams')" class="mb-2 h-8 text-sm" />
											<div class="max-h-52 space-y-0.5 overflow-y-auto">
												<button v-for="t in filteredTeams" :key="t.id" type="button" class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent" :class="selectedTeamId === t.id ? 'bg-accent text-accent-foreground' : 'text-foreground'" @click="selectedTeamId = t.id; teamPickerOpen = false; teamSearch = ''">
													<Check v-if="selectedTeamId === t.id" class="h-3.5 w-3.5 shrink-0" />
													<span v-else class="h-3.5 w-3.5 shrink-0" />
													<span class="truncate">{{ t.name }}</span>
												</button>
											</div>
										</PopoverContent>
									</Popover>
									<ShadButton :disabled="!selectedTeamId" size="sm" class="w-full" @click="assignTeam">{{ tx('settings.budgets.assign_team', 'Assign team') }}</ShadButton>
								</div>

								<div class="space-y-1.5">
									<ShadLabel>{{ tx('settings.budgets.assign_user', 'Assign user') }}</ShadLabel>
									<Popover v-model:open="userPickerOpen">
										<PopoverTrigger as-child>
											<ShadButton variant="outline" class="w-full justify-between font-normal" @click="userPickerOpen = true">
												<span class="truncate" :class="selectedUserId ? 'text-foreground' : 'text-muted-foreground'">
													{{ selectedUserId ? (users.find(u => u.id === selectedUserId)?.label ?? selectedUserId) : tx('settings.budgets.pick_user', 'Select a user') }}
												</span>
												<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 text-muted-foreground" />
											</ShadButton>
										</PopoverTrigger>
										<PopoverContent class="w-72 p-2" align="start">
											<ShadInput v-model="userSearch" :placeholder="tx('settings.admin_users.search', 'Search users')" class="mb-2 h-8 text-sm" />
											<div class="max-h-52 space-y-0.5 overflow-y-auto">
												<button v-for="u in filteredUsers" :key="u.id" type="button" class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent" :class="selectedUserId === u.id ? 'bg-accent text-accent-foreground' : 'text-foreground'" @click="selectedUserId = u.id; userPickerOpen = false; userSearch = ''">
													<Check v-if="selectedUserId === u.id" class="h-3.5 w-3.5 shrink-0" />
													<span v-else class="h-3.5 w-3.5 shrink-0" />
													<span class="truncate">{{ u.label }}</span>
												</button>
											</div>
										</PopoverContent>
									</Popover>
									<ShadButton :disabled="!selectedUserId" size="sm" class="w-full" @click="assignUser">{{ tx('settings.budgets.assign_user', 'Assign user') }}</ShadButton>
								</div>
							</div>
						</div>
					</div>
				</div>
			</ShadTabsContent>

			<ShadTabsContent value="history" class="mt-0">
				<div class="overflow-hidden rounded-lg border border-border bg-card">
					<div class="grid grid-cols-[10rem_1fr_1fr_1fr_8rem_10rem] gap-3 border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
						<span>{{ tx('settings.budgets.reset_at', 'Reset at') }}</span>
						<span>{{ tx('settings.budgets.scope', 'Scope') }}</span>
						<span>{{ tx('settings.budgets.budget', 'Budget') }}</span>
						<span>{{ tx('settings.budgets.reason', 'Reason') }}</span>
						<span>{{ tx('settings.budgets.kind', 'Kind') }}</span>
						<span>{{ tx('settings.budgets.reset_by', 'Reset by') }}</span>
					</div>
					<div v-if="budgetStore.resetHistory.length === 0" class="p-6 text-center text-sm text-muted-foreground">
						{{ tx('settings.budgets.no_reset_history', 'No reset history yet') }}
					</div>
					<div v-for="event in budgetStore.resetHistory" v-else :key="event.id" class="grid grid-cols-[10rem_1fr_1fr_1fr_8rem_10rem] gap-3 border-b border-border px-4 py-3 text-sm last:border-b-0">
						<span class="text-xs text-muted-foreground">{{ formatDate(event.reset_at) }}</span>
						<span class="truncate">{{ resetScopeLabel(event) }}</span>
						<span class="truncate">{{ event.budget_name ?? '-' }}</span>
						<span class="truncate text-muted-foreground">{{ event.reason ?? '-' }}</span>
						<span>{{ event.kind ?? '-' }}</span>
						<span class="truncate">{{ event.created_by_label ?? '-' }}</span>
					</div>
				</div>
			</ShadTabsContent>
		</ShadTabs>

		<ShadDialog v-model:open="resetOpen">
			<ShadDialogContent class="sm:max-w-[460px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ tx('settings.budgets.reset_budget', 'Reset budget') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ resetLabel }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div class="space-y-4">
					<div class="space-y-1.5">
						<ShadLabel>{{ tx('settings.budgets.kind_scope', 'Kind') }}</ShadLabel>
						<ShadSelect v-model="resetKind">
							<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem value="all">{{ tx('settings.budgets.all_kinds', 'All kinds') }}</ShadSelectItem>
								<ShadSelectItem value="pooled">pooled</ShadSelectItem>
								<ShadSelectItem value="per_user">per_user</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div class="space-y-1.5">
						<ShadLabel>{{ tx('settings.budgets.reason', 'Reason') }}</ShadLabel>
						<ShadTextarea v-model="resetReason" rows="3" :placeholder="tx('settings.budgets.reason_placeholder', 'Optional note for the audit log')" />
					</div>
					<p class="text-sm text-muted-foreground">{{ tx('settings.budgets.reset_keeps_history', 'Usage history is kept. This starts budget accounting from now for the selected scope.') }}</p>
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="resetOpen = false">{{ tx('common.cancel', 'Cancel') }}</ShadButton>
					<ShadButton variant="destructive" class="gap-2" @click="confirmReset">
						<RotateCcw class="h-4 w-4" />
						<span>{{ tx('settings.budgets.reset', 'Reset') }}</span>
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {Building2, Check, ChevronDown, ChevronsUpDown, Loader2, Plus, RefreshCcw, RotateCcw, Trash2, User, X} from 'lucide-vue-next';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';
import {useMainStore} from '@/stores';
import {useBudgetStore} from '@/stores/budgetStore';
import type {Budget, BudgetAssignmentInfo, BudgetPayload, BudgetResetEvent, BudgetResetPayload, EffectiveBudget, UserBudgetOverview} from '~/types/budgets';
import type {TeamList} from '~/types/chat';

interface UserEntry {
	id: string;
	label: string;
}

const store = useMainStore();
const budgetStore = useBudgetStore();
const {$customFetch} = useNuxtApp();

const loading = ref(false);
const activeTab = ref('users');
const selected = ref<Budget | null>(null);
const teams = ref<TeamList[]>([]);
const users = ref<UserEntry[]>([]);
const expandedUsers = ref(new Set<string>());

const teamPickerOpen = ref(false);
const userPickerOpen = ref(false);
const teamSearch = ref('');
const userSearch = ref('');
const userOverviewSearch = ref('');
const teamOverviewSearch = ref('');
const selectedTeamId = ref<string | null>(null);
const selectedUserId = ref<string | null>(null);
const resetOpen = ref(false);
const resetPayload = ref<BudgetResetPayload>({});
const resetLabel = ref('');
const resetReason = ref('');
const resetKind = ref<'all' | 'pooled' | 'per_user'>('all');

const form = reactive<BudgetPayload & {description: string | null}>({
	name: '',
	description: null,
	amount: '10.0000',
	kind: 'pooled',
	interval: 'monthly',
	reset_strategy: 'calendar',
	on_exceed: 'block',
	is_enabled: true,
});

const canEdit = computed(() => store.hasPermission('admin.budgets.edit'));

const filteredTeams = computed(() => {
	const q = teamSearch.value.toLowerCase();
	return q ? teams.value.filter(t => t.name.toLowerCase().includes(q)) : teams.value;
});

const filteredUsers = computed(() => {
	const q = userSearch.value.toLowerCase();
	return q ? users.value.filter(u => u.label.toLowerCase().includes(q)) : users.value;
});

const filteredUserOverview = computed(() => {
	const q = userOverviewSearch.value.trim().toLowerCase();
	return q ? budgetStore.userOverview.filter(row => `${row.user_label} ${row.teams.map(team => team.name).join(' ')}`.toLowerCase().includes(q)) : budgetStore.userOverview;
});

const filteredTeamOverview = computed(() => {
	const q = teamOverviewSearch.value.trim().toLowerCase();
	return q ? budgetStore.teamOverview.filter(row => row.team_name.toLowerCase().includes(q)) : budgetStore.teamOverview;
});

function tx(key: string, fallback: string): string {
	const value = store.getTranslation(key);
	return value === key ? fallback : value;
}

function fillForm(budget: Budget) {
	form.name = budget.name;
	form.description = budget.description;
	form.amount = budget.amount;
	form.kind = budget.kind;
	form.interval = budget.interval;
	form.reset_strategy = budget.reset_strategy;
	form.on_exceed = budget.on_exceed;
	form.is_enabled = budget.is_enabled;
}

async function selectBudget(budget: Budget) {
	selected.value = budget;
	fillForm(budget);
	selectedTeamId.value = null;
	selectedUserId.value = null;
	await budgetStore.fetchAssignments(budget.id);
}

function formatMoney(value: string | number) {
	return `$${Number(value).toFixed(2)}`;
}

function formatDate(value: string) {
	return new Intl.DateTimeFormat(undefined, {dateStyle: 'medium', timeStyle: 'short'}).format(new Date(value));
}

function decisionClass(decision: string) {
	if (decision === 'block') return 'bg-destructive/10 text-destructive';
	if (decision === 'warn') return 'bg-amber-500/10 text-amber-600';
	return 'bg-green-500/10 text-green-600';
}

function sourceLabel(budget: EffectiveBudget) {
	return budget.team_id ? tx('settings.budgets.team_budget', 'team budget') : tx('settings.budgets.user_budget', 'user budget');
}

function toggleUser(userId: string) {
	const next = new Set(expandedUsers.value);
	if (next.has(userId)) next.delete(userId);
	else next.add(userId);
	expandedUsers.value = next;
}

function openReset(payload: BudgetResetPayload, label: string) {
	resetPayload.value = payload;
	resetLabel.value = label;
	resetReason.value = '';
	resetKind.value = 'all';
	resetOpen.value = true;
}

function resetUserRow(row: UserBudgetOverview) {
	if (row.budgets.length === 1) {
		const budget = row.budgets[0];
		openReset({assignment_id: budget.assignment_id, user_id: row.user_id}, `${row.user_label} · ${budget.budget.name}`);
		return;
	}
	openReset({user_id: row.user_id}, row.user_label);
}

async function confirmReset() {
	try {
		await budgetStore.resetBudget({
			...resetPayload.value,
			kind: resetKind.value === 'all' ? null : resetKind.value,
			reason: resetReason.value || null,
		});
		resetOpen.value = false;
		if (selected.value) await budgetStore.fetchAssignments(selected.value.id);
		store.toast(tx('settings.budgets.reset_success', 'Budget reset'), {type: 'success'});
	} catch {
		store.toast(tx('settings.budgets.reset_error', 'Failed to reset budget'), {type: 'error'});
	}
}

function resetScopeLabel(event: BudgetResetEvent) {
	if (event.assignment_id) return tx('settings.budgets.assignment', 'Assignment');
	if (event.user_label) return event.user_label;
	if (event.team_name) return event.team_name;
	return tx('settings.budgets.budget', 'Budget');
}

async function createBudget() {
	try {
		const budget = await budgetStore.createBudget({...form, name: form.name || store.getTranslation('settings.tabs.budgets')});
		await selectBudget(budget);
		await loadOperationalData();
		store.toast(tx('settings.budgets.create_success', 'Budget created'), {type: 'success'});
	} catch {
		store.toast(tx('settings.budgets.create_error', 'Failed to create budget'), {type: 'error'});
	}
}

async function saveBudget() {
	if (!selected.value) return;
	try {
		const budget = await budgetStore.updateBudget(selected.value.id, {...form});
		await selectBudget(budget);
		await loadOperationalData();
		store.toast(tx('settings.budgets.save_success', 'Budget saved'), {type: 'success'});
	} catch {
		store.toast(tx('settings.budgets.save_error', 'Failed to save budget'), {type: 'error'});
	}
}

async function deleteBudget() {
	if (!selected.value) return;
	try {
		await budgetStore.deleteBudget(selected.value.id);
		selected.value = budgetStore.budgets[0] ?? null;
		if (selected.value) await selectBudget(selected.value);
		await loadOperationalData();
		store.toast(tx('settings.budgets.delete_success', 'Budget deleted'), {type: 'success'});
	} catch {
		store.toast(tx('settings.budgets.delete_error', 'Failed to delete budget'), {type: 'error'});
	}
}

async function assignTeam() {
	if (!selected.value || !selectedTeamId.value) return;
	try {
		await budgetStore.assignBudget(selected.value.id, {team_id: selectedTeamId.value});
		selectedTeamId.value = null;
		await loadOperationalData();
		store.toast(tx('settings.budgets.assign_success', 'Budget assigned'), {type: 'success'});
	} catch {
		store.toast(tx('settings.budgets.assign_error', 'Failed to assign budget'), {type: 'error'});
	}
}

async function assignUser() {
	if (!selected.value || !selectedUserId.value) return;
	try {
		await budgetStore.assignBudget(selected.value.id, {user_id: selectedUserId.value});
		selectedUserId.value = null;
		await loadOperationalData();
		store.toast(tx('settings.budgets.assign_success', 'Budget assigned'), {type: 'success'});
	} catch {
		store.toast(tx('settings.budgets.assign_error', 'Failed to assign budget'), {type: 'error'});
	}
}

async function removeAssignment(a: BudgetAssignmentInfo) {
	if (!selected.value) return;
	try {
		await budgetStore.removeAssignment(selected.value.id, a.id);
		await loadOperationalData();
		store.toast(tx('settings.budgets.remove_success', 'Assignment removed'), {type: 'success'});
	} catch {
		store.toast(tx('settings.budgets.remove_error', 'Failed to remove assignment'), {type: 'error'});
	}
}

async function loadTeamsAndUsers() {
	const [teamsRes, usersRes] = await Promise.all([
		$customFetch<{items: TeamList[]; has_more: boolean}>('/api/v1/admin/teams', {params: {size: 200}}),
		$customFetch<{users: {id: string; username: string; email: string}[]; total: number}>('/api/v1/admin/users', {params: {per_page: 200}}),
	]);
	teams.value = teamsRes?.items ?? [];
	users.value = (usersRes?.users ?? []).map(u => ({id: u.id, label: u.username || u.email}));
}

async function loadOperationalData() {
	await Promise.all([
		budgetStore.fetchUserOverview(),
		budgetStore.fetchTeamOverview(),
		budgetStore.fetchResetHistory(),
	]);
}

onMounted(async () => {
	loading.value = true;
	try {
		await Promise.all([budgetStore.fetchBudgets(), loadTeamsAndUsers(), loadOperationalData()]);
		if (budgetStore.budgets[0]) await selectBudget(budgetStore.budgets[0]);
	} finally {
		loading.value = false;
	}
});
</script>
