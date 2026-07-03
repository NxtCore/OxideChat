CREATE TABLE IF NOT EXISTS teams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    allow_all_models BOOLEAN NOT NULL DEFAULT false,
    budget_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS teams_single_default ON teams (is_default) WHERE is_default = true;

CREATE TABLE IF NOT EXISTS team_members (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (team_id, user_id)
);

CREATE TABLE IF NOT EXISTS team_model_access (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    provider_id UUID REFERENCES providers(id) ON DELETE CASCADE,
    model_id UUID REFERENCES models(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT team_model_access_target_check CHECK (
        (provider_id IS NOT NULL AND model_id IS NULL) OR
        (provider_id IS NULL AND model_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS team_model_access_provider_unique ON team_model_access (team_id, provider_id) WHERE provider_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS team_model_access_model_unique ON team_model_access (team_id, model_id) WHERE model_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS team_permissions (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    is_allowed BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (team_id, permission_id)
);

CREATE INDEX IF NOT EXISTS idx_teams_default ON teams(is_default) WHERE is_default = true;
CREATE INDEX IF NOT EXISTS idx_team_members_team ON team_members(team_id);
CREATE INDEX IF NOT EXISTS idx_team_members_user ON team_members(user_id);
CREATE INDEX IF NOT EXISTS idx_team_model_access_team ON team_model_access(team_id);
CREATE INDEX IF NOT EXISTS idx_team_model_access_provider ON team_model_access(provider_id);
CREATE INDEX IF NOT EXISTS idx_team_model_access_model ON team_model_access(model_id);
CREATE INDEX IF NOT EXISTS idx_team_permissions_permission ON team_permissions(permission_id);

INSERT INTO permissions (name, description) VALUES
    ('admin.teams.view', 'View teams'),
    ('admin.teams.edit', 'Manage teams')
ON CONFLICT (name) DO NOTHING;

INSERT INTO teams (name, description, is_default, allow_all_models)
VALUES ('General', 'Default team for all users', true, true)
ON CONFLICT (name) DO UPDATE
    SET is_default = true,
        allow_all_models = true,
        updated_at = NOW();

INSERT INTO team_members (team_id, user_id)
SELECT t.id, u.id
FROM teams t
CROSS JOIN users u
WHERE t.name = 'General'
ON CONFLICT DO NOTHING;

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.tabs.teams', 'Teams'),
    ('en', 'settings.teams.description', 'Organize users, model access, and budgets'),
    ('en', 'settings.teams.create', 'Create Team'),
    ('en', 'settings.teams.search', 'Search teams...'),
    ('en', 'settings.teams.default', 'Default'),
    ('en', 'settings.teams.members_count', '{count} members'),
    ('en', 'settings.teams.name', 'Name'),
    ('en', 'settings.teams.description_field', 'Description'),
    ('en', 'settings.teams.allow_all_models', 'Allow all models'),
    ('en', 'settings.teams.allow_all_models_hint', 'Members can use every enabled model. Turn off to restrict access.'),
    ('en', 'settings.teams.members', 'Members'),
    ('en', 'settings.teams.models', 'Models'),
    ('en', 'settings.teams.search_members', 'Search members...'),
    ('en', 'settings.teams.search_models', 'Search models...'),
    ('en', 'settings.teams.providers', 'Providers'),
    ('en', 'settings.teams.budget', 'Budget'),
    ('en', 'settings.teams.no_members', 'No users found'),
    ('en', 'settings.teams.no_models', 'No models found'),
    ('en', 'settings.teams.selected_count', '{count} selected'),
    ('en', 'settings.teams.no_selection_title', 'No team selected'),
    ('en', 'settings.teams.no_selection', 'Select a team to manage its members, models, and budget.'),
    ('en', 'settings.teams.confirm_delete', 'Delete this team? Members keep access through their other teams.'),
    ('en', 'settings.teams.load_error', 'Failed to load teams'),
    ('en', 'settings.teams.create_success', 'Team created'),
    ('en', 'settings.teams.create_error', 'Failed to create team'),
    ('en', 'settings.teams.save_success', 'Team saved'),
    ('en', 'settings.teams.save_error', 'Failed to save team'),
    ('en', 'settings.teams.delete_success', 'Team deleted'),
    ('en', 'settings.teams.delete_error', 'Failed to delete team'),
    ('en', 'settings.teams.budget_link_title', 'Budget configuration'),
    ('en', 'settings.teams.budget_link_hint', 'Link this team to a budget configuration to track and enforce usage limits.'),
    ('en', 'settings.teams.budget_not_linked', 'No budget linked'),
    ('en', 'settings.teams.budget_linked', 'Linked budget'),
    ('en', 'settings.teams.budget_none_available', 'No budget configurations exist yet. Once created, you can link one here.'),
    ('en', 'settings.teams.budget_unlink', 'Unlink'),
    ('en', 'settings.admin_users.field_teams', 'Teams'),
    ('en', 'settings.admin_users.select_teams', 'Select teams'),
    ('en', 'settings.admin_users.filter_team', 'Filter team'),
    ('de', 'settings.tabs.teams', 'Teams'),
    ('de', 'settings.teams.description', 'Benutzer, Modellzugriff und Budgets organisieren'),
    ('de', 'settings.teams.create', 'Team erstellen'),
    ('de', 'settings.teams.search', 'Teams suchen...'),
    ('de', 'settings.teams.default', 'Standard'),
    ('de', 'settings.teams.members_count', '{count} Mitglieder'),
    ('de', 'settings.teams.name', 'Name'),
    ('de', 'settings.teams.description_field', 'Beschreibung'),
    ('de', 'settings.teams.allow_all_models', 'Alle Modelle erlauben'),
    ('de', 'settings.teams.allow_all_models_hint', 'Mitglieder können jedes aktivierte Modell nutzen. Deaktivieren, um den Zugriff einzuschränken.'),
    ('de', 'settings.teams.members', 'Mitglieder'),
    ('de', 'settings.teams.models', 'Modelle'),
    ('de', 'settings.teams.search_members', 'Mitglieder suchen...'),
    ('de', 'settings.teams.search_models', 'Modelle suchen...'),
    ('de', 'settings.teams.providers', 'Anbieter'),
    ('de', 'settings.teams.budget', 'Budget'),
    ('de', 'settings.teams.no_members', 'Keine Benutzer gefunden'),
    ('de', 'settings.teams.no_models', 'Keine Modelle gefunden'),
    ('de', 'settings.teams.selected_count', '{count} ausgewählt'),
    ('de', 'settings.teams.no_selection_title', 'Kein Team ausgewählt'),
    ('de', 'settings.teams.no_selection', 'Wähle ein Team, um Mitglieder, Modelle und Budget zu verwalten.'),
    ('de', 'settings.teams.confirm_delete', 'Dieses Team löschen? Mitglieder behalten den Zugriff über ihre anderen Teams.'),
    ('de', 'settings.teams.load_error', 'Teams konnten nicht geladen werden'),
    ('de', 'settings.teams.create_success', 'Team erstellt'),
    ('de', 'settings.teams.create_error', 'Team konnte nicht erstellt werden'),
    ('de', 'settings.teams.save_success', 'Team gespeichert'),
    ('de', 'settings.teams.save_error', 'Team konnte nicht gespeichert werden'),
    ('de', 'settings.teams.delete_success', 'Team gelöscht'),
    ('de', 'settings.teams.delete_error', 'Team konnte nicht gelöscht werden'),
    ('de', 'settings.teams.budget_link_title', 'Budget-Konfiguration'),
    ('de', 'settings.teams.budget_link_hint', 'Verknüpfe dieses Team mit einer Budget-Konfiguration, um Nutzungslimits zu verfolgen und durchzusetzen.'),
    ('de', 'settings.teams.budget_not_linked', 'Kein Budget verknüpft'),
    ('de', 'settings.teams.budget_linked', 'Verknüpftes Budget'),
    ('de', 'settings.teams.budget_none_available', 'Es existieren noch keine Budget-Konfigurationen. Sobald eine erstellt wurde, kannst du sie hier verknüpfen.'),
    ('de', 'settings.teams.budget_unlink', 'Trennen'),
    ('de', 'settings.admin_users.field_teams', 'Teams'),
    ('de', 'settings.admin_users.select_teams', 'Teams auswählen'),
    ('de', 'settings.admin_users.filter_team', 'Nach Team filtern')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW();
