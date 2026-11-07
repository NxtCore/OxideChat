import type {Browser, Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {MEMBER, MOCK_IDP_URL} from './support/constants';

const SLUG = 'e2e-idp';
const NAME = 'E2E IdP';
const SECRET = 'e2e-idp-client-secret-value';

type Profile = {sub: string; email: string; email_verified: boolean | null; name: string};

async function setProfile(profile: Profile) {
	await fetch(`${MOCK_IDP_URL}/profile`, {method: 'POST', body: JSON.stringify(profile)});
}

async function signInWithProvider(browser: Browser, baseURL: string, check: (page: Page) => Promise<void>) {
	const visitor = await browser.newContext({baseURL, storageState: {cookies: [], origins: []}});
	try {
		const page = await visitor.newPage();
		await page.goto('/auth/login');
		await page.getByTestId(`oauth-button-${SLUG}`).click();
		await check(page);
	} finally {
		await visitor.close();
	}
}

async function expectSignedIn(page: Page, baseURL: string, text: string) {
	await expect(page).toHaveURL(url => url.origin === new URL(baseURL).origin && !url.pathname.startsWith('/auth'));
	expect(JSON.stringify(await (await page.request.get('/api/v1/users/@me')).json())).toContain(text);
}

test.describe.configure({mode: 'serial'});

test.describe('custom sign-in providers', () => {
	test('an admin adds a provider through discovery and its secret is never returned', async ({page, api}) => {
		await page.goto('/settings/admin-config');
		await page.getByTestId('oauth-add').click();
		await page.getByTestId('oauth-name').fill(NAME);
		await page.getByTestId('oauth-slug').fill(SLUG);
		await page.getByTestId('oauth-issuer').fill(MOCK_IDP_URL);
		await page.getByTestId('oauth-client-id').fill('e2e-client');
		await expect(page.getByTestId('oauth-save')).toBeDisabled();
		await page.getByTestId('oauth-client-secret').fill(SECRET);
		await expect(page.getByTestId('oauth-callback-url')).toContainText(`/api/v1/auth/oauth/${SLUG}/callback`);
		await page.getByTestId('oauth-save').click();
		await expect(page.getByTestId('oauth-client-id')).toBeHidden();

		const providers = await api.get('/api/v1/admin/oauth-providers');
		expect(providers.find((provider: {slug: string}) => provider.slug === SLUG)).toMatchObject({
			name: NAME,
			client_id: 'e2e-client',
			endpoints: {issuer_url: MOCK_IDP_URL, authorization_url: `${MOCK_IDP_URL}/authorize`, token_url: `${MOCK_IDP_URL}/token`, userinfo_url: `${MOCK_IDP_URL}/userinfo`},
			trust_email: false,
			allow_registration: true,
			enabled: true,
		});
		expect(JSON.stringify(providers)).not.toContain(SECRET);
		expect((await api.get('/api/v1/base')).oauth_providers).toContainEqual({slug: SLUG, name: NAME, icon: null});
		await expect(page.getByTestId(`oauth-configure-${SLUG}`)).toBeVisible();
	});

	test('a new person signs in with PKCE and gets an account', async ({browser, baseURL}) => {
		await setProfile({sub: 'idp-new', email: 'new-idp@e2e.test', email_verified: true, name: 'Idp New'});
		await signInWithProvider(browser, baseURL!, async page => {
			await expectSignedIn(page, baseURL!, 'new-idp@e2e.test');
		});
		const token = await (await fetch(`${MOCK_IDP_URL}/last-token-request`)).json();
		expect(token).toMatchObject({grant_type: 'authorization_code', code: 'mock-code', has_code_verifier: true});
	});

	test('an unverified email is refused with an explanation', async ({browser, baseURL, t}) => {
		await setProfile({sub: 'idp-unverified', email: 'unverified@e2e.test', email_verified: false, name: 'Unverified'});
		await signInWithProvider(browser, baseURL!, async page => {
			await expect(page).toHaveURL(/\/auth\/error\?code=oauth_email_not_verified/);
			await expect(page.getByText(t('auth.errors.oauth_email_not_verified_title'))).toBeVisible();
		});
	});

	test('a verified email links to the existing account', async ({browser, baseURL}) => {
		await setProfile({sub: 'idp-member', email: MEMBER.email, email_verified: true, name: 'Someone Else'});
		await signInWithProvider(browser, baseURL!, async page => {
			await expectSignedIn(page, baseURL!, MEMBER.username);
		});
	});

	test('a provider that forbids sign-up still lets known people in', async ({browser, baseURL, api, t}) => {
		const saved = (await api.get('/api/v1/admin/oauth-providers')).find((provider: {slug: string}) => provider.slug === SLUG);
		await api.put(`/api/v1/admin/oauth-providers/${saved.id}`, {...saved, client_secret: null, allow_registration: false});

		await setProfile({sub: 'idp-stranger', email: 'stranger@e2e.test', email_verified: true, name: 'Stranger'});
		await signInWithProvider(browser, baseURL!, async page => {
			await expect(page).toHaveURL(/\/auth\/error\?code=oauth_registration_closed/);
			await expect(page.getByText(t('auth.errors.oauth_registration_closed_title'))).toBeVisible();
		});

		await setProfile({sub: 'idp-new', email: 'new-idp@e2e.test', email_verified: true, name: 'Idp New'});
		await signInWithProvider(browser, baseURL!, async page => {
			await expectSignedIn(page, baseURL!, 'new-idp@e2e.test');
		});
	});

	test('addresses that reach cloud metadata are rejected', async ({page}) => {
		const response = await page.request.post('/api/v1/admin/oauth-providers', {
			data: {
				slug: 'e2e-metadata',
				name: 'Metadata',
				icon: null,
				endpoints: {issuer_url: 'http://169.254.169.254', authorization_url: null, token_url: null, userinfo_url: null},
				client_id: 'x',
				client_secret: 'y',
				scopes: ['openid'],
				claim_mapping: {subject: 'sub', email: 'email', email_verified: 'email_verified', name: 'name', avatar: 'picture', groups: 'groups'},
				trust_email: false,
				allow_registration: true,
				enabled: true,
			},
		});
		expect(response.status()).toBe(400);
	});

	test('an admin removes the provider and its button disappears', async ({page, api}) => {
		await page.goto('/settings/admin-config');
		await page.getByTestId(`oauth-configure-${SLUG}`).click();
		await page.getByTestId('oauth-remove').click();
		await expect(page.getByTestId('oauth-client-id')).toBeHidden();

		expect((await api.get('/api/v1/admin/oauth-providers')).some((provider: {slug: string}) => provider.slug === SLUG)).toBe(false);
		expect((await api.get('/api/v1/base')).oauth_providers).toEqual([]);
		await expect(page.getByTestId('oauth-empty')).toBeVisible();
	});
});
