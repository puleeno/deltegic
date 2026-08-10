use crate::Result;
use reqwest::{Client, header::HeaderMap};
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::info;

/// Generic HTTP downloader with progress tracking
pub struct HttpDownloader {
    client: Client,
}

impl HttpDownloader {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        Ok(Self { client })
    }

    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    pub async fn download_to_file(
        &self,
        url: &str,
        output_path: &Path,
        headers: Option<HeaderMap>,
        on_progress: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<u64> {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut request = self.client.get(url);
        if let Some(h) = headers {
            request = request.headers(h);
        }

        let response = request.send().await?;
        let total = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;

        let mut file = fs::File::create(output_path).await?;
        let mut stream = response.bytes_stream();

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            on_progress(downloaded, total);
        }

        file.flush().await?;
        info!("Downloaded {} bytes to {:?}", downloaded, output_path);
        Ok(downloaded)
    }

    pub async fn fetch_text(&self, url: &str, headers: Option<HeaderMap>) -> Result<String> {
        let mut request = self.client.get(url);
        if let Some(h) = headers {
            request = request.headers(h);
        }
        let text = request.send().await?.text().await?;
        Ok(text)
    }

    pub async fn fetch_json(&self, url: &str, headers: Option<HeaderMap>) -> Result<serde_json::Value> {
        let mut request = self.client.get(url);
        if let Some(h) = headers {
            request = request.headers(h);
        }
        let json: serde_json::Value = request.send().await?.json().await?;
        Ok(json)
    }

    pub async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
        headers: Option<HeaderMap>,
    ) -> Result<serde_json::Value> {
        let mut request = self.client.post(url).json(body);
        if let Some(h) = headers {
            request = request.headers(h);
        }
        let json: serde_json::Value = request.send().await?.json().await?;
        Ok(json)
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl Default for HttpDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP downloader")
    }
}
