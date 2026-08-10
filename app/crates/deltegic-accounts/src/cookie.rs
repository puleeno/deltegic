use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single cookie entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub expires: Option<DateTime<Utc>>,
}

impl CookieEntry {
    pub fn new(name: impl Into<String>, value: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
            path: "/".to_string(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires.map(|e| e < Utc::now()).unwrap_or(false)
    }

    /// Format as Cookie header value
    pub fn header_value(&self) -> String {
        format!("{}={}", self.name, self.value)
    }
}

/// Netscape/JSON cookie jar for a specific account+domain scope
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CookieJar {
    /// domain -> cookie_name -> cookie
    cookies: HashMap<String, HashMap<String, CookieEntry>>,
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a cookie
    pub fn set(&mut self, cookie: CookieEntry) {
        self.cookies
            .entry(cookie.domain.clone())
            .or_default()
            .insert(cookie.name.clone(), cookie);
    }

    /// Get cookies for a domain (including parent domains)
    pub fn get_for_domain(&self, domain: &str) -> Vec<&CookieEntry> {
        let mut result = Vec::new();
        for (cookie_domain, cookies) in &self.cookies {
            let normalized = cookie_domain.trim_start_matches('.');
            if domain == normalized || domain.ends_with(&format!(".{}", normalized)) {
                for cookie in cookies.values() {
                    if !cookie.is_expired() {
                        result.push(cookie);
                    }
                }
            }
        }
        result
    }

    /// Build a Cookie header string for a domain
    pub fn cookie_header(&self, domain: &str) -> String {
        self.get_for_domain(domain)
            .iter()
            .map(|c| c.header_value())
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Parse Netscape cookie file format
    pub fn from_netscape(content: &str) -> Self {
        let mut jar = CookieJar::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                let entry = CookieEntry {
                    domain: parts[0].to_string(),
                    path: parts[2].to_string(),
                    secure: parts[3].eq_ignore_ascii_case("TRUE"),
                    expires: parts[4].parse::<i64>().ok().and_then(|ts| {
                        if ts == 0 {
                            None
                        } else {
                            DateTime::from_timestamp(ts, 0)
                        }
                    }),
                    name: parts[5].to_string(),
                    value: parts[6].to_string(),
                    http_only: parts[0].starts_with("#HttpOnly_"),
                    same_site: None,
                };
                jar.set(entry);
            }
        }
        jar
    }

    /// Export as Netscape format
    pub fn to_netscape(&self) -> String {
        let mut lines = vec!["# Netscape HTTP Cookie File".to_string()];
        for cookies in self.cookies.values() {
            for cookie in cookies.values() {
                let expires = cookie
                    .expires
                    .map(|e| e.timestamp().to_string())
                    .unwrap_or_else(|| "0".to_string());
                lines.push(format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    cookie.domain,
                    if cookie.domain.starts_with('.') { "TRUE" } else { "FALSE" },
                    cookie.path,
                    if cookie.secure { "TRUE" } else { "FALSE" },
                    expires,
                    cookie.name,
                    cookie.value,
                ));
            }
        }
        lines.join("\n")
    }

    /// Parse from browser JSON export (e.g. EditThisCookie)
    pub fn from_json_export(json: &serde_json::Value) -> Self {
        let mut jar = CookieJar::new();
        if let Some(arr) = json.as_array() {
            for item in arr {
                if let (Some(name), Some(value), Some(domain)) = (
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("value").and_then(|v| v.as_str()),
                    item.get("domain").and_then(|v| v.as_str()),
                ) {
                    let mut entry = CookieEntry::new(name, value, domain);
                    if let Some(path) = item.get("path").and_then(|v| v.as_str()) {
                        entry.path = path.to_string();
                    }
                    if let Some(secure) = item.get("secure").and_then(|v| v.as_bool()) {
                        entry.secure = secure;
                    }
                    if let Some(http_only) = item.get("httpOnly").and_then(|v| v.as_bool()) {
                        entry.http_only = http_only;
                    }
                    if let Some(exp) = item.get("expirationDate").and_then(|v| v.as_f64()) {
                        entry.expires = DateTime::from_timestamp(exp as i64, 0);
                    }
                    jar.set(entry);
                }
            }
        }
        jar
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    pub fn count(&self) -> usize {
        self.cookies.values().map(|m| m.len()).sum()
    }
}
