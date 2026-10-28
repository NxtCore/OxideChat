INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.errors.team_admin_required', 'Only team admins can view service-account request logs'),
    ('de', 'developer.errors.team_admin_required', 'Nur Team-Admins können Anfrageprotokolle von Service-Accounts einsehen')
ON CONFLICT (language, key_path) DO NOTHING;
