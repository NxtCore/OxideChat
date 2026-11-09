import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';
import {MEMBER, MOCK_LLM_KEY, MOCK_LLM_URL} from './support/constants';

test.describe('provider routing', () => {
	test('admins sync a gateway and pick its providers from the list', async ({page, api, t}) => {
		const before = (await api.get('/api/v1/config')).provider_routing;
		const provider = await api.post('/api/v1/admin/providers', {kind: 'OPENROUTER', name: uniqueName('Router'), base_url: MOCK_LLM_URL, api_key: MOCK_LLM_KEY});
		const syncedAt = async () => (await api.get('/api/v1/admin/upstream-providers')).find((gateway: {kind: string}) => gateway.kind === 'OPENROUTER')?.synced_at ?? null;
		try {
			await api.patch('/api/v1/admin/config', {provider_routing: {...before, allowed_providers: {}}});
			const previous = await syncedAt();
			await page.goto('/settings/admin-config');
			await expect(page.getByTestId('config-allowed-providers-openrouter-synced')).toBeVisible();
			await page.getByTestId('config-allowed-providers-openrouter-sync').click();
			await expect.poll(syncedAt).not.toBe(previous);
			await expect(page.getByTestId('config-allowed-providers-openrouter-synced')).not.toHaveText(t('settings.provider_allowlist.not_synced'));

			await page.getByTestId('config-allowed-providers-openrouter-restrict').click();
			await expect(page.getByTestId('config-allowed-providers-openrouter-empty')).toBeVisible();
			await page.getByTestId('config-allowed-providers-openrouter-option-azure').getByRole('checkbox').click();
			await page.getByTestId('config-allowed-providers-openrouter-input').fill('mistral');
			await page.getByTestId('config-allowed-providers-openrouter-input').press('Enter');
			await expect(page.getByTestId('config-allowed-providers-openrouter-empty')).toBeHidden();
			await expect(page.getByTestId('config-allowed-providers-gateway-openrouter-status')).toHaveText(t('settings.provider_allowlist.count_allowed', {count: 2}));

			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByTestId('settings-save-bar')).toBeHidden();
			expect((await api.get('/api/v1/config')).provider_routing.allowed_providers).toEqual({OPENROUTER: ['azure', 'mistral']});
		} finally {
			await api.patch('/api/v1/admin/config', {provider_routing: before});
			await api.delete(`/api/v1/admin/providers/${provider.id}`);
		}
	});

	test('a team picks its providers within the installation list', async ({page, api, t}) => {
		const before = (await api.get('/api/v1/config')).provider_routing;
		const name = uniqueName('RoutingTeam');
		const team = await api.post('/api/v1/admin/teams', {name});
		const gateway = page.getByTestId('team-allowed-providers-gateway-openrouter');
		const openGateway = async () => {
			if ((await gateway.getAttribute('aria-expanded')) !== 'true') await gateway.click();
		};
		try {
			await api.patch('/api/v1/admin/config', {provider_routing: {...before, allowed_providers: {OPENROUTER: ['azure', 'together']}}});
			await page.goto('/settings/teams');
			await page.getByRole('button', {name}).click();
			await page.getByTestId('team-tab-routing').click();

			await page.getByTestId('team-provider-selector').click();
			await page.getByRole('option', {name: t('settings.teams.provider_selector.off')}).click();
			await openGateway();
			await page.getByTestId('team-allowed-providers-openrouter-restrict').click();
			await expect(page.getByTestId('team-allowed-providers-openrouter-option-azure')).toBeVisible();
			await expect(page.getByTestId('team-allowed-providers-openrouter-option-together')).toBeVisible();
			await expect(page.getByTestId('team-allowed-providers-openrouter-option-mistral')).toHaveCount(0);

			await page.getByTestId('team-allowed-providers-openrouter-input').fill('openai');
			await expect(page.getByTestId('team-allowed-providers-openrouter-add')).toHaveCount(0);
			await page.getByTestId('team-allowed-providers-openrouter-input').fill('');
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.locator('[data-sonner-toast]').filter({hasText: t('settings.provider_allowlist.empty')})).toBeVisible();
			await expect(page.getByTestId('settings-save-bar')).toBeVisible();
			expect((await api.get(`/api/v1/admin/teams/${team.id}`)).provider_routing.allowed_providers).toEqual({});

			await page.getByTestId('team-allowed-providers-openrouter-option-together').getByRole('checkbox').click();

			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByTestId('settings-save-bar')).toBeHidden();
			const saved = await api.get(`/api/v1/admin/teams/${team.id}`);
			expect(saved.provider_routing).toEqual({selector_enabled: false, allowed_providers: {OPENROUTER: ['together']}});

			await page.reload();
			await page.getByRole('button', {name}).click();
			await page.getByTestId('team-tab-routing').click();
			await expect(page.getByTestId('team-provider-selector')).toContainText(t('settings.teams.provider_selector.off'));
			await openGateway();
			await expect(page.getByTestId('team-allowed-providers-openrouter-option-together').getByRole('checkbox')).toBeChecked();
			await expect(page.getByTestId('team-allowed-providers-openrouter-option-azure').getByRole('checkbox')).not.toBeChecked();
		} finally {
			await api.patch('/api/v1/admin/config', {provider_routing: before});
		}
	});

	test('a team selector override applies right after logging in', async ({browser, baseURL, api, seed, t}) => {
		const before = (await api.get('/api/v1/config')).provider_routing;
		const team = await api.post('/api/v1/admin/teams', {name: uniqueName('SelectorTeam'), member_ids: [seed.memberId]});
		const visitor = await browser.newContext({baseURL, storageState: {cookies: [], origins: []}});
		const page = await visitor.newPage();
		const selectorEnabled = () =>
			page.evaluate(() => (document.querySelector('#__nuxt') as any).__vue_app__.config.globalProperties.$pinia.state.value.main.base?.chat_provider_selector?.enabled ?? null);
		try {
			await api.patch('/api/v1/admin/config', {provider_routing: {...before, selector_enabled: false}});
			await api.put(`/api/v1/admin/teams/${team.id}`, {
				name: team.name,
				description: null,
				allow_all_models: true,
				default_model_id: null,
				members: [{user_id: seed.memberId, role: 'member'}],
				provider_ids: [],
				model_ids: [],
				provider_routing: {selector_enabled: true, allowed_providers: {}},
			});
			await page.goto('/auth/login');
			await page.getByLabel(t('auth.login.email')).fill(MEMBER.email);
			expect(await selectorEnabled()).toBe(false);
			await page.getByLabel(t('auth.login.password')).fill(MEMBER.password);
			await page.getByRole('button', {name: t('auth.login.submit')}).click();
			await expect(page).not.toHaveURL(/\/auth\//);
			await expect.poll(selectorEnabled).toBe(true);
		} finally {
			await visitor.close();
			await api.patch('/api/v1/admin/config', {provider_routing: before});
			await api.delete(`/api/v1/admin/teams/${team.id}`);
		}
	});

	test('empty, malformed or misplaced allowlists are refused', async ({api}) => {
		const before = (await api.get('/api/v1/config')).provider_routing;
		for (const allowed_providers of [{OPENROUTER: []}, {OPENROUTER: ['not a slug']}, {OPENAI: ['azure']}]) {
			await expect(api.patch('/api/v1/admin/config', {provider_routing: {selector_enabled: false, allowed_providers}})).rejects.toThrow(/-> 400/);
		}
		expect((await api.get('/api/v1/config')).provider_routing).toEqual(before);
	});
});
