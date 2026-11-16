INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'auth.errors.registration_disabled', 'Registration is turned off on this installation'),
    ('de', 'auth.errors.registration_disabled', 'Die Registrierung ist auf dieser Installation deaktiviert'),
    ('en', 'settings.admin_config.registration', 'Allow registration'),
    ('de', 'settings.admin_config.registration', 'Registrierung erlauben'),
    ('en', 'settings.admin_config.registration_hint', 'Visitors can create an account with email and password. Sign-in providers have their own setting. New accounts join the default team and get its model access.'),
    ('de', 'settings.admin_config.registration_hint', 'Besucher können ein Konto mit E-Mail und Passwort anlegen. Anmeldeanbieter haben eine eigene Einstellung. Neue Konten kommen ins Standardteam und erhalten dessen Modellzugriff.')
ON CONFLICT (language, key_path) DO NOTHING;

UPDATE images
SET source = 'icon'
WHERE source IS DISTINCT FROM 'model_icon'
  AND (
    '/api/v1/images/' || id::text IN (
        SELECT icon FROM model_configs WHERE icon IS NOT NULL
        UNION
        SELECT icon FROM model_presets WHERE icon IS NOT NULL
    )
    OR id::text = (SELECT value FROM app_config WHERE key = 'instance_icon_id')
  );
