INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.models.editor.variables_panel_title', 'Available Variables'),
    ('en', 'settings.models.editor.variables_panel_desc', 'These placeholders are replaced at runtime when the system prompt is sent to the model.'),
    ('en', 'settings.models.editor.var_user_name', 'Username of the logged-in user'),
    ('en', 'settings.models.editor.var_user_email', 'Email address of the logged-in user'),
    ('en', 'settings.models.editor.var_date', 'Current UTC date (YYYY-MM-DD)'),
    ('en', 'settings.models.editor.var_time', 'Current UTC time (HH:MM)'),
    ('en', 'settings.models.editor.var_datetime', 'Current UTC date and time (YYYY-MM-DD HH:MM)'),
    ('en', 'settings.models.editor.var_model_name', 'Display name of the model'),
    ('en', 'settings.models.editor.var_model_id', 'Identifier string of the model'),

    ('de', 'settings.models.editor.variables_panel_title', 'Verfügbare Variablen'),
    ('de', 'settings.models.editor.variables_panel_desc', 'Diese Platzhalter werden beim Senden des System-Prompts an das Modell ersetzt.'),
    ('de', 'settings.models.editor.var_user_name', 'Benutzername des eingeloggten Nutzers'),
    ('de', 'settings.models.editor.var_user_email', 'E-Mail-Adresse des eingeloggten Nutzers'),
    ('de', 'settings.models.editor.var_date', 'Aktuelles UTC-Datum (JJJJ-MM-TT)'),
    ('de', 'settings.models.editor.var_time', 'Aktuelle UTC-Zeit (HH:MM)'),
    ('de', 'settings.models.editor.var_datetime', 'Aktuelles UTC-Datum und Uhrzeit (JJJJ-MM-TT HH:MM)'),
    ('de', 'settings.models.editor.var_model_name', 'Anzeigename des Modells'),
    ('de', 'settings.models.editor.var_model_id', 'Bezeichner-String des Modells')

    ON CONFLICT (language, key_path) DO NOTHING;
