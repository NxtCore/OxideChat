-- Add admin users translations (English + German)
-- Inserts only the missing keys used by the admin users UI

INSERT INTO i18n_translations (language, key_path, value) VALUES
    -- Admin Users UI
    ('en', 'settings.admin_users.create_user', 'Create User'),
    ('de', 'settings.admin_users.create_user', 'Benutzer erstellen'),

    ('en', 'settings.admin_users.search_placeholder', 'Search users...'),
    ('de', 'settings.admin_users.search_placeholder', 'Benutzer suchen...'),

    ('en', 'settings.admin_users.filter_role', 'Filter role'),
    ('de', 'settings.admin_users.filter_role', 'Nach Rolle filtern'),

    ('en', 'settings.admin_users.role_all', 'All'),
    ('de', 'settings.admin_users.role_all', 'Alle'),

    ('en', 'settings.admin_users.no_users', 'No users found'),
    ('de', 'settings.admin_users.no_users', 'Keine Benutzer gefunden'),

    ('en', 'settings.admin_users.cannot_modify_self', 'You cannot modify your own account'),
    ('de', 'settings.admin_users.cannot_modify_self', 'Sie können Ihr eigenes Konto nicht bearbeiten'),

    ('en', 'settings.admin_users.create_user_description', 'Create a new user with optional roles and password'),
    ('de', 'settings.admin_users.create_user_description', 'Erstellen Sie einen neuen Benutzer mit optionalen Rollen und Passwort'),

    ('en', 'settings.admin_users.field_email', 'Email'),
    ('de', 'settings.admin_users.field_email', 'E-Mail'),

    ('en', 'settings.admin_users.field_username', 'Username'),
    ('de', 'settings.admin_users.field_username', 'Benutzername'),

    ('en', 'settings.admin_users.field_password', 'Password'),
    ('de', 'settings.admin_users.field_password', 'Passwort'),

    ('en', 'settings.admin_users.field_roles', 'Roles'),
    ('de', 'settings.admin_users.field_roles', 'Rollen'),

    ('en', 'settings.admin_users.edit_user', 'Edit User'),
    ('de', 'settings.admin_users.edit_user', 'Benutzer bearbeiten'),

    ('en', 'settings.admin_users.reset_password', 'Reset Password'),
    ('de', 'settings.admin_users.reset_password', 'Passwort zurücksetzen'),

    ('en', 'settings.admin_users.reset_password_placeholder', 'New password (leave blank to keep current)'),
    ('de', 'settings.admin_users.reset_password_placeholder', 'Neues Passwort (leer lassen, um aktuelles zu behalten)'),

    ('en', 'settings.admin_users.delete_user', 'Delete User'),
    ('de', 'settings.admin_users.delete_user', 'Benutzer löschen'),

    ('en', 'settings.admin_users.delete_confirm', 'Are you sure you want to delete {username}? This action cannot be undone.'),
    ('de', 'settings.admin_users.delete_confirm', 'Möchten Sie {username} wirklich löschen? Diese Aktion kann nicht rückgängig gemacht werden.'),

    -- Toasts / Errors
    ('en', 'settings.admin_users.load_error', 'Failed to load users'),
    ('de', 'settings.admin_users.load_error', 'Benutzer konnten nicht geladen werden'),

    ('en', 'settings.admin_users.create_success', 'User created'),
    ('de', 'settings.admin_users.create_success', 'Benutzer erstellt'),

    ('en', 'settings.admin_users.create_error', 'Failed to create user'),
    ('de', 'settings.admin_users.create_error', 'Fehler beim Erstellen des Benutzers'),

    ('en', 'settings.admin_users.edit_success', 'User updated'),
    ('de', 'settings.admin_users.edit_success', 'Benutzer aktualisiert'),

    ('en', 'settings.admin_users.edit_error', 'Failed to update user'),
    ('de', 'settings.admin_users.edit_error', 'Fehler beim Aktualisieren des Benutzers'),

    ('en', 'settings.admin_users.delete_success', 'User deleted'),
    ('de', 'settings.admin_users.delete_success', 'Benutzer gelöscht'),

    ('en', 'settings.admin_users.delete_error', 'Failed to delete user'),
    ('de', 'settings.admin_users.delete_error', 'Fehler beim Löschen des Benutzers'),

    ('en', 'settings.admin_users.select_roles', 'Select roles'),
    ('de', 'settings.admin_users.select_roles', 'Rollen auswählen'),
    -- Common
    ('en', 'common.create', 'Create'),
    ('de', 'common.create', 'Erstellen'),
    ('en', 'common.next', 'Next'),
    ('de', 'common.next', 'Nächste'),
    ('en', 'common.previous', 'Previous'),
    ('de', 'common.previous', 'Vorherige')
ON CONFLICT (language, key_path) DO NOTHING;
