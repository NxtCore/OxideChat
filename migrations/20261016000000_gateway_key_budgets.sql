ALTER TABLE budget_assignments ADD COLUMN IF NOT EXISTS api_key_id UUID REFERENCES gateway_api_keys(id) ON DELETE CASCADE;

ALTER TABLE budget_assignments DROP CONSTRAINT IF EXISTS budget_assignments_target_check;
ALTER TABLE budget_assignments ADD CONSTRAINT budget_assignments_target_check CHECK (
    num_nonnulls(team_id, user_id, api_key_id) = 1
);

CREATE UNIQUE INDEX IF NOT EXISTS budget_assignments_api_key_unique
    ON budget_assignments (api_key_id)
    WHERE api_key_id IS NOT NULL;

CREATE OR REPLACE FUNCTION delete_api_key_budget() RETURNS trigger AS $$
BEGIN
    DELETE FROM budgets WHERE id = OLD.budget_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS budget_assignments_delete_api_key_budget ON budget_assignments;
CREATE TRIGGER budget_assignments_delete_api_key_budget
    AFTER DELETE ON budget_assignments
    FOR EACH ROW
    WHEN (OLD.api_key_id IS NOT NULL)
    EXECUTE FUNCTION delete_api_key_budget();

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.errors.budget_invalid', 'The budget needs a positive amount, a supported interval and a block or warn action'),
    ('en', 'gateway.errors.budget_unavailable', 'Budget enforcement is temporarily unavailable'),
    ('de', 'developer.errors.budget_invalid', 'Das Budget braucht einen positiven Betrag, ein unterstütztes Intervall und die Aktion Blockieren oder Warnen'),
    ('de', 'gateway.errors.budget_unavailable', 'Die Budgetprüfung ist vorübergehend nicht verfügbar')
ON CONFLICT (language, key_path) DO NOTHING;
