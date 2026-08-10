use crate::{cookie::CookieJar, AccountError, Result};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use tracing::{info, warn};
use uuid::Uuid;

pub type AccountId = Uuid;

/// An account for a website / service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    /// Friendly label (e.g. "My Facebook", "Work Instagram")
    pub label: String,
    /// Site/service this account is for (e.g. "facebook.com")
    pub site: String,
    /// Username / email (optional, display only)
    pub username: Option<String>,
    /// Cookie jar for this account (stored encrypted at rest)
    pub cookies: CookieJar,
    /// Extra headers to inject (e.g. Authorization)
    pub extra_headers: std::collections::HashMap<String, String>,
    /// Whether to use this account for matching URLs automatically
    pub auto_match: bool,
    /// URL patterns this account applies to (regex or glob)
    pub url_patterns: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
}

impl Account {
    pub fn new(label: impl Into<String>, site: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            site: site.into(),
            username: None,
            cookies: CookieJar::new(),
            extra_headers: Default::default(),
            auto_match: true,
            url_patterns: Vec::new(),
            created_at: now,
            updated_at: now,
            last_used: None,
        }
    }

    pub fn with_cookies(mut self, jar: CookieJar) -> Self {
        self.cookies = jar;
        self
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn with_url_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.url_patterns.push(pattern.into());
        self
    }

    pub fn cookie_header(&self, domain: &str) -> String {
        self.cookies.cookie_header(domain)
    }
}

/// Encrypted account data for persistence
#[derive(Serialize, Deserialize)]
struct EncryptedAccount {
    id: AccountId,
    label: String,
    site: String,
    nonce: String,  // hex
    ciphertext: String,  // hex
}

/// Persistent, encrypted account store
pub struct AccountStore {
    accounts: Arc<DashMap<AccountId, Account>>,
    storage_path: PathBuf,
    cipher: Option<Aes256Gcm>,
}

impl AccountStore {
    /// Create a store with optional encryption key (hex string or raw passphrase)
    pub fn new(storage_path: PathBuf, passphrase: Option<&str>) -> Self {
        let cipher = passphrase.map(|pw| {
            let mut hasher = Sha256::new();
            hasher.update(pw.as_bytes());
            let key_bytes = hasher.finalize();
            let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
            Aes256Gcm::new(key)
        });

        let store = Self {
            accounts: Arc::new(DashMap::new()),
            storage_path,
            cipher,
        };
        store
    }

    /// Load accounts from disk
    pub async fn load(&self) -> Result<()> {
        if !self.storage_path.exists() {
            return Ok(());
        }
        let data = tokio::fs::read_to_string(&self.storage_path).await?;
        let encrypted_list: Vec<EncryptedAccount> = serde_json::from_str(&data)?;

        for enc in encrypted_list {
            match self.decrypt_account(&enc) {
                Ok(account) => {
                    self.accounts.insert(account.id, account);
                }
                Err(e) => {
                    warn!("Failed to decrypt account {}: {}", enc.id, e);
                }
            }
        }
        info!("Loaded {} accounts", self.accounts.len());
        Ok(())
    }

    /// Persist accounts to disk
    pub async fn save(&self) -> Result<()> {
        if let Some(parent) = self.storage_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let encrypted_list: Vec<EncryptedAccount> = self.accounts
            .iter()
            .filter_map(|entry| self.encrypt_account(entry.value()).ok())
            .collect();

        let data = serde_json::to_string_pretty(&encrypted_list)?;
        tokio::fs::write(&self.storage_path, data).await?;
        Ok(())
    }

    pub fn add(&self, account: Account) -> AccountId {
        let id = account.id;
        self.accounts.insert(id, account);
        id
    }

    pub fn get(&self, id: &AccountId) -> Option<Account> {
        self.accounts.get(id).map(|a| a.clone())
    }

    pub fn list(&self) -> Vec<Account> {
        self.accounts.iter().map(|a| a.clone()).collect()
    }

    pub fn update(&self, account: Account) -> Result<()> {
        if self.accounts.contains_key(&account.id) {
            self.accounts.insert(account.id, account);
            Ok(())
        } else {
            Err(AccountError::NotFound(account.id))
        }
    }

    pub fn remove(&self, id: &AccountId) -> Option<Account> {
        self.accounts.remove(id).map(|(_, a)| a)
    }

    /// Find best matching account for a URL
    pub fn find_for_url(&self, url: &str) -> Option<Account> {
        let parsed = url::Url::parse(url).ok()?;
        let domain = parsed.domain()?;

        // First try pattern matching
        for account in self.accounts.iter() {
            if !account.auto_match { continue; }
            for pattern in &account.url_patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(url) {
                        return Some(account.clone());
                    }
                }
                // Simple glob: check domain suffix
                if domain.ends_with(pattern.trim_start_matches('*').trim_start_matches('.')) {
                    return Some(account.clone());
                }
            }
        }

        // Fallback: site-based matching
        for account in self.accounts.iter() {
            if !account.auto_match { continue; }
            let site = account.site.trim_start_matches('.');
            if domain.ends_with(site) || domain == site {
                return Some(account.clone());
            }
        }

        None
    }

    fn encrypt_account(&self, account: &Account) -> Result<EncryptedAccount> {
        let plain = serde_json::to_vec(account)?;
        if let Some(cipher) = &self.cipher {
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
            let ciphertext = cipher
                .encrypt(&nonce, plain.as_slice())
                .map_err(|e| AccountError::Encryption(e.to_string()))?;
            Ok(EncryptedAccount {
                id: account.id,
                label: account.label.clone(),
                site: account.site.clone(),
                nonce: hex::encode(nonce),
                ciphertext: hex::encode(ciphertext),
            })
        } else {
            // Unencrypted
            Ok(EncryptedAccount {
                id: account.id,
                label: account.label.clone(),
                site: account.site.clone(),
                nonce: String::new(),
                ciphertext: hex::encode(&plain),
            })
        }
    }

    fn decrypt_account(&self, enc: &EncryptedAccount) -> Result<Account> {
        let ciphertext = hex::decode(&enc.ciphertext)
            .map_err(|e| AccountError::Encryption(e.to_string()))?;

        if let Some(cipher) = &self.cipher {
            let nonce_bytes = hex::decode(&enc.nonce)
                .map_err(|e| AccountError::Encryption(e.to_string()))?;
            let nonce = Nonce::from_slice(&nonce_bytes);
            let plain = cipher
                .decrypt(nonce, ciphertext.as_slice())
                .map_err(|e| AccountError::Encryption(e.to_string()))?;
            Ok(serde_json::from_slice(&plain)?)
        } else {
            Ok(serde_json::from_slice(&ciphertext)?)
        }
    }
}
