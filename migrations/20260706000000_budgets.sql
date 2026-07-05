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
