-- Add OAuth error translations and new error codes

-- OAuth email conflict error (when OAuth email matches existing account)
INSERT INTO translations (language, key, value) VALUES
    ('en', 'auth.errors.oauth_email_conflict', 'An account with this email already exists. Please log in with your original method.'),
    ('en', 'auth.errors.oauth_email_conflict_title', 'Account Already Exists'),
    ('en', 'auth.errors.oauth_email_conflict_help', 'Your OAuth provider returned an email that is already associated with an existing account. To link your OAuth identity, please log in with your existing credentials first, then link the OAuth provider from your account settings.'),
    ('en', 'auth.errors.oauth_state_mismatch', 'OAuth authentication failed due to a state mismatch. Please try again.'),
    ('en', 'auth.errors.oauth_state_mismatch_title', 'Authentication Failed'),
    ('en', 'auth.errors.oauth_token_error', 'Failed to obtain authentication token. Please try again.'),
    ('en', 'auth.errors.oauth_token_error_title', 'Token Error'),
    ('en', 'auth.errors.oauth_user_info_error', 'Failed to retrieve user information from the OAuth provider.'),
    ('en', 'auth.errors.oauth_user_info_error_title', 'User Info Error'),
    ('en', 'auth.errors.unknown_error', 'An unexpected error occurred. Please try again.'),
    ('en', 'auth.errors.unknown_error_title', 'Error'),
    ('en', 'auth.errors.back_to_login', 'Back to Login'),
    ('en', 'auth.errors.go_home', 'Go Home'),
    ('en', 'auth.errors.email_taken', 'This email is already registered'),
    ('en', 'auth.errors.username_taken', 'This username is already taken'),
    
    ('de', 'auth.errors.oauth_email_conflict', 'Ein Konto mit dieser E-Mail-Adresse existiert bereits. Bitte melden Sie sich mit Ihrer ursprünglichen Methode an.'),
    ('de', 'auth.errors.oauth_email_conflict_title', 'Konto existiert bereits'),
    ('de', 'auth.errors.oauth_email_conflict_help', 'Ihr OAuth-Anbieter hat eine E-Mail-Adresse zurückgegeben, die bereits mit einem bestehenden Konto verknüpft ist. Um Ihre OAuth-Identität zu verknüpfen, melden Sie sich bitte zuerst mit Ihren bestehenden Anmeldedaten an und verknüpfen Sie dann den OAuth-Anbieter in Ihren Kontoeinstellungen.'),
    ('de', 'auth.errors.oauth_state_mismatch', 'OAuth-Authentifizierung aufgrund einer Statusabweichung fehlgeschlagen. Bitte versuchen Sie es erneut.'),
    ('de', 'auth.errors.oauth_state_mismatch_title', 'Authentifizierung fehlgeschlagen'),
    ('de', 'auth.errors.oauth_token_error', 'Authentifizierungstoken konnte nicht abgerufen werden. Bitte versuchen Sie es erneut.'),
    ('de', 'auth.errors.oauth_token_error_title', 'Token-Fehler'),
    ('de', 'auth.errors.oauth_user_info_error', 'Benutzerinformationen konnten nicht vom OAuth-Anbieter abgerufen werden.'),
    ('de', 'auth.errors.oauth_user_info_error_title', 'Benutzerinfo-Fehler'),
    ('de', 'auth.errors.unknown_error', 'Ein unerwarteter Fehler ist aufgetreten. Bitte versuchen Sie es erneut.'),
    ('de', 'auth.errors.unknown_error_title', 'Fehler'),
    ('de', 'auth.errors.back_to_login', 'Zurück zur Anmeldung'),
    ('de', 'auth.errors.go_home', 'Zur Startseite'),
    ('de', 'auth.errors.email_taken', 'Diese E-Mail-Adresse ist bereits registriert'),
    ('de', 'auth.errors.username_taken', 'Dieser Benutzername ist bereits vergeben')
ON CONFLICT (language, key) DO UPDATE SET value = EXCLUDED.value;
