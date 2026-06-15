use std::sync::{Arc, Mutex};

const SERVICE: &str = "hush-windows";
const ACCOUNT: &str = "auth-token";

#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    SignedOut,
    SignedIn { token: String },
}

pub struct AuthStore {
    state: Arc<Mutex<AuthState>>,
}

impl AuthStore {
    pub fn new() -> Self {
        let state = match Self::load_from_keyring() {
            Some(token) => AuthState::SignedIn { token },
            None => AuthState::SignedOut,
        };
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn state(&self) -> AuthState {
        self.state.lock().unwrap().clone()
    }

    pub fn token(&self) -> Option<String> {
        match self.state.lock().unwrap().clone() {
            AuthState::SignedIn { token } => Some(token),
            AuthState::SignedOut => None,
        }
    }

    pub fn is_signed_in(&self) -> bool {
        matches!(*self.state.lock().unwrap(), AuthState::SignedIn { .. })
    }

    pub fn sign_in(&self, token: String) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) {
            let _ = entry.set_password(&token);
        }
        *self.state.lock().unwrap() = AuthState::SignedIn { token };
        log::info!("Signed in, token stored in Credential Manager");
    }

    pub fn sign_out(&self) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) {
            let _ = entry.delete_credential();
        }
        *self.state.lock().unwrap() = AuthState::SignedOut;
        log::info!("Signed out, token removed from Credential Manager");
    }

    fn load_from_keyring() -> Option<String> {
        let entry = keyring::Entry::new(SERVICE, ACCOUNT).ok()?;
        entry.get_password().ok()
    }
}

impl Default for AuthStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Backend URL — reads HUSH_SERVER_URL env var or falls back to localhost for dev
pub fn backend_url() -> String {
    std::env::var("HUSH_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
}

pub fn pair_url() -> String {
    format!("{}/pair", backend_url())
}
