DO $$ BEGIN
    CREATE TYPE budget_kind AS ENUM ('pooled', 'per_user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE budget_interval AS ENUM ('daily', 'weekly', 'monthly', 'total');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE budget_reset_strategy AS ENUM ('calendar', 'rolling', 'anchored');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE budget_on_exceed AS ENUM ('block', 'warn', 'allow');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS budgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(120) NOT NULL,
    description TEXT,
    amount NUMERIC(12,4) NOT NULL,
    kind budget_kind NOT NULL,
    interval budget_interval NOT NULL,
    reset_strategy budget_reset_strategy NOT NULL DEFAULT 'calendar',
    on_exceed budget_on_exceed NOT NULL DEFAULT 'block',
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS budget_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    budget_id UUID NOT NULL REFERENCES budgets(id) ON DELETE CASCADE,
    team_id UUID REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT budget_assignments_target_check CHECK (
        (team_id IS NOT NULL AND user_id IS NULL) OR
        (team_id IS NULL AND user_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS budget_assignments_team_budget_unique
    ON budget_assignments (team_id, budget_id)
    WHERE team_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS budget_assignments_user_budget_unique
    ON budget_assignments (user_id, budget_id)
    WHERE user_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS usage_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    team_id UUID REFERENCES teams(id) ON DELETE SET NULL,
    model_id UUID REFERENCES models(id) ON DELETE SET NULL,
    provider_id UUID REFERENCES providers(id) ON DELETE SET NULL,
    request_type VARCHAR(50) NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
    cost_total NUMERIC(12,6) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_usage_events_user_created ON usage_events(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_usage_events_model_created ON usage_events(model_id, created_at);
CREATE INDEX IF NOT EXISTS idx_usage_events_team_created ON usage_events(team_id, created_at);

CREATE TABLE IF NOT EXISTS model_pricing_overrides (
    model_id UUID PRIMARY KEY REFERENCES models(id) ON DELETE CASCADE,
    pricing JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO permissions (name, description) VALUES
    ('admin.budgets.view', 'View budget configurations'),
    ('admin.budgets.edit', 'Manage budget configurations'),
    ('admin.analytics.view', 'View usage analytics')
ON CONFLICT (name) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'admin'
  AND p.name IN ('admin.budgets.view', 'admin.budgets.edit', 'admin.analytics.view')
ON CONFLICT DO NOTHING;

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.tabs.budgets', 'Budgets'),
    ('en', 'settings.tabs.analytics', 'Analytics'),
    ('en', 'settings.budgets.description', 'Limit spend for users and teams'),
    ('en', 'settings.budgets.create', 'Create budget'),
    ('en', 'settings.budgets.name', 'Name'),
    ('en', 'settings.budgets.amount', 'Amount'),
    ('en', 'settings.budgets.kind', 'Kind'),
    ('en', 'settings.budgets.interval', 'Interval'),
    ('en', 'settings.budgets.reset_strategy', 'Reset'),
    ('en', 'settings.budgets.on_exceed', 'Action'),
    ('en', 'settings.budgets.enabled', 'Enabled'),
    ('en', 'settings.budgets.assign_team', 'Assign team'),
    ('en', 'settings.budgets.assign_user', 'Assign user'),
    ('en', 'settings.budgets.used', 'Used'),
    ('en', 'settings.budgets.remaining', 'Remaining'),
    ('en', 'settings.budgets.notes', 'Description'),
    ('en', 'settings.budgets.assignments', 'Assignments'),
    ('en', 'settings.budgets.no_assignments', 'No assignments yet'),
    ('en', 'settings.budgets.pick_team', 'Select a team…'),
    ('en', 'settings.budgets.pick_user', 'Select a user…'),
    ('en', 'settings.budgets.reached', 'Budget reached'),
    ('en', 'settings.analytics.total_cost', 'Total cost'),
    ('en', 'settings.analytics.total_tokens', 'Total tokens'),
    ('en', 'settings.analytics.avg_cost', 'Avg. cost / day'),
    ('en', 'settings.analytics.top_models', 'By model'),
    ('en', 'settings.analytics.top_users', 'By user'),
    ('en', 'settings.analytics.top_teams', 'By team'),
    ('en', 'settings.analytics.spend_over_time', 'Spend over time'),
    ('en', 'settings.analytics.token_split', 'Token split'),
    ('en', 'settings.analytics.label', 'Label'),
    ('en', 'settings.analytics.last_7d', 'Last 7 days'),
    ('en', 'settings.analytics.last_30d', 'Last 30 days'),
    ('en', 'settings.analytics.last_90d', 'Last 90 days'),
    ('en', 'settings.analytics.custom', 'Custom'),
    ('en', 'settings.analytics.description', 'Review token and spend trends'),
    ('en', 'settings.analytics.group_by', 'Group by'),
    ('en', 'settings.analytics.from', 'From'),
    ('en', 'settings.analytics.to', 'To'),
    ('en', 'settings.analytics.cost', 'Cost'),
    ('en', 'settings.analytics.tokens', 'Tokens'),
    ('en', 'settings.models.editor.pricing', 'Pricing'),
    ('en', 'settings.models.editor.pricing_desc', 'Override accounting prices per one million tokens.'),
    ('en', 'settings.models.editor.reported_price', 'Reported price'),
    ('en', 'settings.models.editor.effective_price', 'Effective price'),
    ('en', 'settings.models.editor.override_input', 'Input override'),
    ('en', 'settings.models.editor.override_output', 'Output override'),
    ('en', 'settings.models.editor.override_reasoning', 'Reasoning override'),
    ('en', 'settings.models.editor.override_cache_read', 'Cache read override'),
    ('en', 'settings.models.editor.override_cache_write', 'Cache write override'),
    ('en', 'settings.models.editor.pricing_unit', 'Rates are per one million tokens. Leave optional fields empty to keep them unset.'),
    ('en', 'settings.models.editor.pricing_saved', 'Pricing saved'),
    ('en', 'settings.models.editor.pricing_deleted', 'Pricing override removed'),
    ('en', 'sidebar.budget.remaining', '{amount} left'),
    ('de', 'settings.tabs.budgets', 'Budgets'),
    ('de', 'settings.tabs.analytics', 'Analysen'),
    ('de', 'settings.budgets.description', 'Ausgaben fuer Benutzer und Teams begrenzen'),
    ('de', 'settings.budgets.create', 'Budget erstellen'),
    ('de', 'settings.budgets.name', 'Name'),
    ('de', 'settings.budgets.amount', 'Betrag'),
    ('de', 'settings.budgets.kind', 'Art'),
    ('de', 'settings.budgets.interval', 'Intervall'),
    ('de', 'settings.budgets.reset_strategy', 'Zuruecksetzen'),
    ('de', 'settings.budgets.on_exceed', 'Aktion'),
    ('de', 'settings.budgets.enabled', 'Aktiv'),
    ('de', 'settings.budgets.assign_team', 'Team zuweisen'),
    ('de', 'settings.budgets.assign_user', 'Benutzer zuweisen'),
    ('de', 'settings.budgets.used', 'Verbraucht'),
    ('de', 'settings.budgets.remaining', 'Verbleibend'),
    ('de', 'settings.budgets.notes', 'Beschreibung'),
    ('de', 'settings.budgets.assignments', 'Zuweisungen'),
    ('de', 'settings.budgets.no_assignments', 'Noch keine Zuweisungen'),
    ('de', 'settings.budgets.pick_team', 'Team auswaehlen…'),
    ('de', 'settings.budgets.pick_user', 'Benutzer auswaehlen…'),
    ('de', 'settings.budgets.reached', 'Budget erreicht'),
    ('de', 'settings.analytics.total_cost', 'Gesamtkosten'),
    ('de', 'settings.analytics.total_tokens', 'Gesamte Tokens'),
    ('de', 'settings.analytics.avg_cost', 'Durchschn. Kosten / Tag'),
    ('de', 'settings.analytics.top_models', 'Nach Modell'),
    ('de', 'settings.analytics.top_users', 'Nach Benutzer'),
    ('de', 'settings.analytics.top_teams', 'Nach Team'),
    ('de', 'settings.analytics.spend_over_time', 'Ausgaben ueber Zeit'),
    ('de', 'settings.analytics.token_split', 'Token-Aufteilung'),
    ('de', 'settings.analytics.label', 'Bezeichnung'),
    ('de', 'settings.analytics.last_7d', 'Letzte 7 Tage'),
    ('de', 'settings.analytics.last_30d', 'Letzte 30 Tage'),
    ('de', 'settings.analytics.last_90d', 'Letzte 90 Tage'),
    ('de', 'settings.analytics.custom', 'Benutzerdefiniert'),
    ('de', 'settings.analytics.description', 'Token- und Ausgabentrends pruefen'),
    ('de', 'settings.analytics.group_by', 'Gruppieren nach'),
    ('de', 'settings.analytics.from', 'Von'),
    ('de', 'settings.analytics.to', 'Bis'),
    ('de', 'settings.analytics.cost', 'Kosten'),
    ('de', 'settings.analytics.tokens', 'Tokens'),
    ('de', 'settings.models.editor.pricing', 'Preise'),
    ('de', 'settings.models.editor.pricing_desc', 'Abrechnungspreise pro eine Million Tokens ueberschreiben.'),
    ('de', 'settings.models.editor.reported_price', 'Gemeldeter Preis'),
    ('de', 'settings.models.editor.effective_price', 'Effektiver Preis'),
    ('de', 'settings.models.editor.override_input', 'Eingabe-Ueberschreibung'),
    ('de', 'settings.models.editor.override_output', 'Ausgabe-Ueberschreibung'),
    ('de', 'settings.models.editor.override_reasoning', 'Reasoning-Ueberschreibung'),
    ('de', 'settings.models.editor.override_cache_read', 'Cache-Lese-Ueberschreibung'),
    ('de', 'settings.models.editor.override_cache_write', 'Cache-Schreib-Ueberschreibung'),
    ('de', 'settings.models.editor.pricing_unit', 'Preise gelten pro eine Million Tokens. Optionale Felder leer lassen, um sie nicht zu setzen.'),
    ('de', 'settings.models.editor.pricing_saved', 'Preise gespeichert'),
    ('de', 'settings.models.editor.pricing_deleted', 'Preis-Ueberschreibung entfernt'),
    ('de', 'sidebar.budget.remaining', '{amount} uebrig')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW();

INSERT INTO i18n_translations (language, key_path, value)
VALUES
    ('en', 'settings.analytics.my_usage', 'My Usage'),
    ('en', 'settings.analytics.requests', 'Requests'),
    ('en', 'settings.analytics.per_day', 'per day'),
    ('en', 'settings.analytics.usage_by_model', 'Usage by model'),
    ('en', 'settings.analytics.request_volume', 'Request volume by model'),
    ('en', 'settings.analytics.no_data', 'No data available for this period'),
    ('en', 'settings.analytics.trending', 'Trending'),
    ('en', 'settings.analytics.tab_overview', 'Overview'),
    ('en', 'settings.analytics.tab_trends', 'Trends'),
    ('en', 'settings.analytics.tab_explore', 'Explore'),
    ('en', 'settings.analytics.metric', 'Metric'),
    ('en', 'settings.analytics.model', 'Model'),
    ('en', 'settings.analytics.none', 'None'),
    ('en', 'settings.analytics.value', 'Value'),
    ('en', 'settings.analytics.tokens_input', 'Tokens (Input)'),
    ('en', 'settings.analytics.tokens_output', 'Tokens (Output)'),
    ('en', 'settings.analytics.tokens_reasoning', 'Tokens (Reasoning)'),
    ('de', 'settings.analytics.my_usage', 'Meine Nutzung'),
    ('de', 'settings.analytics.requests', 'Anfragen'),
    ('de', 'settings.analytics.per_day', 'pro Tag'),
    ('de', 'settings.analytics.usage_by_model', 'Nutzung nach Modell'),
    ('de', 'settings.analytics.request_volume', 'Anfrageanzahl nach Modell'),
    ('de', 'settings.analytics.no_data', 'Keine Daten fuer diesen Zeitraum verfuegbar'),
    ('de', 'settings.analytics.trending', 'Trend'),
    ('de', 'settings.analytics.tab_overview', 'Uebersicht'),
    ('de', 'settings.analytics.tab_trends', 'Trends'),
    ('de', 'settings.analytics.tab_explore', 'Erkunden'),
    ('de', 'settings.analytics.metric', 'Metrik'),
    ('de', 'settings.analytics.model', 'Modell'),
    ('de', 'settings.analytics.none', 'Keine'),
    ('de', 'settings.analytics.value', 'Wert'),
    ('de', 'settings.analytics.tokens_input', 'Tokens (Eingabe)'),
    ('de', 'settings.analytics.tokens_output', 'Tokens (Ausgabe)'),
    ('de', 'settings.analytics.tokens_reasoning', 'Tokens (Reasoning)')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;

INSERT INTO i18n_translations (language, key_path, value)
VALUES
    ('en', 'settings.analytics.top_api_keys', 'Top API Keys'),
    ('en', 'settings.analytics.coming_soon', 'Coming soon'),
    ('en', 'settings.analytics.api_keys_soon', 'API key tracking will be available in a future release'),
    ('en', 'settings.analytics.total_tokens_short', 'Tokens'),
    ('en', 'settings.analytics.custom_range', 'Custom range'),
    ('en', 'settings.analytics.preset_last_7d', 'Last 7 days'),
    ('en', 'settings.analytics.preset_last_30d', 'Last 30 days'),
    ('en', 'settings.analytics.preset_last_90d', 'Last 90 days'),
    ('en', 'settings.analytics.preset_this_month', 'This month'),
    ('en', 'settings.analytics.preset_last_month', 'Last month'),
    ('en', 'settings.analytics.preset_this_year', 'This year'),
    ('de', 'settings.analytics.top_api_keys', 'Top API-Schlüssel'),
    ('de', 'settings.analytics.coming_soon', 'Demnächst'),
    ('de', 'settings.analytics.api_keys_soon', 'API-Schlüssel-Tracking wird in einem zukünftigen Release verfügbar sein'),
    ('de', 'settings.analytics.total_tokens_short', 'Tokens'),
    ('de', 'settings.analytics.custom_range', 'Benutzerdefinierter Zeitraum'),
    ('de', 'settings.analytics.preset_last_7d', 'Letzte 7 Tage'),
    ('de', 'settings.analytics.preset_last_30d', 'Letzte 30 Tage'),
    ('de', 'settings.analytics.preset_last_90d', 'Letzte 90 Tage'),
    ('de', 'settings.analytics.preset_this_month', 'Diesen Monat'),
    ('de', 'settings.analytics.preset_last_month', 'Letzten Monat'),
    ('de', 'settings.analytics.preset_this_year', 'Dieses Jahr')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;

INSERT INTO i18n_translations (language, key_path, value)
VALUES
    ('en', 'settings.analytics.spend', 'Spend'),
    ('en', 'settings.analytics.section_models', 'Models'),
    ('en', 'settings.analytics.section_api_keys', 'API Keys'),
    ('en', 'settings.analytics.section_user', 'Users'),
    ('en', 'settings.analytics.trend', 'Trend'),
    ('de', 'settings.analytics.spend', 'Ausgaben'),
    ('de', 'settings.analytics.section_models', 'Modelle'),
    ('de', 'settings.analytics.section_api_keys', 'API-Schlüssel'),
    ('de', 'settings.analytics.section_user', 'Benutzer'),
    ('de', 'settings.analytics.trend', 'Trend')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;

INSERT INTO i18n_translations (language, key_path, value)
VALUES
    ('en', 'settings.analytics.request_count', 'Request count'),
    ('en', 'settings.analytics.total_usage_dollars', 'Total usage ($)'),
    ('en', 'settings.analytics.tokens_total', 'Tokens (total)'),
    ('en', 'settings.analytics.tokens_prompt', 'Tokens (prompt)'),
    ('en', 'settings.analytics.tokens_completion', 'Tokens (completion)'),
    ('en', 'settings.analytics.reasoning_tokens', 'Reasoning tokens'),
    ('en', 'settings.analytics.latency', 'Latency'),
    ('en', 'settings.analytics.api_key', 'API key'),
    ('en', 'settings.analytics.provider', 'Provider'),
    ('en', 'settings.analytics.user', 'User'),
    ('en', 'settings.analytics.top', 'Top'),
    ('en', 'settings.analytics.rollup', 'Rollup'),
    ('en', 'settings.analytics.hourly', 'Hourly'),
    ('en', 'settings.analytics.daily', 'Daily'),
    ('en', 'settings.analytics.weekly', 'Weekly'),
    ('en', 'settings.analytics.total', 'Total'),
    ('en', 'settings.analytics.min', 'Min'),
    ('en', 'settings.analytics.max', 'Max'),
    ('en', 'settings.analytics.avg', 'Avg'),
    ('en', 'settings.analytics.sum', 'Sum'),
    ('en', 'settings.analytics.other', 'Other'),
    ('de', 'settings.analytics.request_count', 'Anfragen'),
    ('de', 'settings.analytics.total_usage_dollars', 'Gesamtnutzung ($)'),
    ('de', 'settings.analytics.tokens_total', 'Tokens (gesamt)'),
    ('de', 'settings.analytics.tokens_prompt', 'Tokens (Prompt)'),
    ('de', 'settings.analytics.tokens_completion', 'Tokens (Completion)'),
    ('de', 'settings.analytics.reasoning_tokens', 'Reasoning-Tokens'),
    ('de', 'settings.analytics.latency', 'Latenz'),
    ('de', 'settings.analytics.api_key', 'API-Schluessel'),
    ('de', 'settings.analytics.provider', 'Provider'),
    ('de', 'settings.analytics.user', 'Benutzer'),
    ('de', 'settings.analytics.top', 'Top'),
    ('de', 'settings.analytics.rollup', 'Rollup'),
    ('de', 'settings.analytics.hourly', 'Stuendlich'),
    ('de', 'settings.analytics.daily', 'Taeglich'),
    ('de', 'settings.analytics.weekly', 'Woechentlich'),
    ('de', 'settings.analytics.total', 'Gesamt'),
    ('de', 'settings.analytics.min', 'Min'),
    ('de', 'settings.analytics.max', 'Max'),
    ('de', 'settings.analytics.avg', 'Durchschn.'),
    ('de', 'settings.analytics.sum', 'Summe'),
    ('de', 'settings.analytics.other', 'Andere')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;
