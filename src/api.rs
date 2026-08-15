use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{self, HeaderMap, HeaderValue};

use crate::models::Inbox;

pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: &str, api_token: &str) -> Self {
        let mut headers = HeaderMap::new();
        let auth_value = HeaderValue::from_str(&format!("Bearer {}", api_token))
            .expect("Invalid API token header value");
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn list_inboxes(&self) -> Result<Vec<Inbox>> {
        let url = format!("{}/api/v1/inboxes", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .context("Failed to connect to API")?;

        if !resp.status().is_success() {
            anyhow::bail!("API returned {}", resp.status());
        }

        parse_json_response(resp, "inboxes")
    }

    pub fn get_inbox(&self, id: u64) -> Result<Inbox> {
        let url = format!("{}/api/v1/inboxes/{}", self.base_url, id);
        let resp = self
            .client
            .get(&url)
            .send()
            .context("Failed to connect to API")?;

        if !resp.status().is_success() {
            anyhow::bail!("API returned {}", resp.status());
        }

        parse_json_response(resp, "inbox")
    }
}

fn parse_json_response<T>(resp: reqwest::blocking::Response, label: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let body = resp
        .text()
        .with_context(|| format!("Failed to read {} response", label))?;

    serde_json::from_str(&body).with_context(|| {
        format!(
            "Failed to parse {} response: {}",
            label,
            body_preview(&body)
        )
    })
}

fn body_preview(body: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 500;

    let preview: String = body.chars().take(MAX_PREVIEW_CHARS).collect();
    if body.chars().count() > MAX_PREVIEW_CHARS {
        format!("{}...", preview)
    } else {
        preview
    }
}
