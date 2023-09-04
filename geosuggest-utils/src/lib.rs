#![doc = include_str!("../README.md")]
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Cursor, Read};

use geosuggest_core::{Engine, SourceFileContentOptions};

pub struct SourceItem<'a> {
    pub url: &'a str,
    pub filename: &'a str,
}

pub struct IndexUpdaterSettings<'a> {
    pub http_timeout_ms: u64,
    pub cities: SourceItem<'a>,
    pub names: Option<SourceItem<'a>>,
    pub countries_url: Option<&'a str>,
    pub admin1_codes_url: Option<&'a str>,
    pub admin2_codes_url: Option<&'a str>,
    pub filter_languages: Vec<&'a str>,
}

impl Default for IndexUpdaterSettings<'_> {
    fn default() -> Self {
        IndexUpdaterSettings {
            http_timeout_ms: 300_000,
            cities: SourceItem {
                url: "http://download.geonames.org/export/dump/cities5000.zip",
                filename: "cities5000.txt",
            },
            names: Some(SourceItem {
                url: "http://download.geonames.org/export/dump/alternateNamesV2.zip",
                filename: "alternateNamesV2.txt",
            }),
            countries_url: Some("http://download.geonames.org/export/dump/countryInfo.txt"),
            admin1_codes_url: Some("http://download.geonames.org/export/dump/admin1CodesASCII.txt"),
            admin2_codes_url: Some("http://download.geonames.org/export/dump/admin2Codes.txt"),
            filter_languages: Vec::new(),
            // max_payload_size: 200 * 1024 * 1024,
        }
    }
}

pub struct IndexUpdater<'a> {
    http_client: reqwest::Client,
    settings: IndexUpdaterSettings<'a>,
}

impl<'a> IndexUpdater<'a> {
    pub fn new(settings: IndexUpdaterSettings<'a>) -> Result<Self> {
        Ok(IndexUpdater {
            http_client: reqwest::ClientBuilder::new()
                .timeout(std::time::Duration::from_millis(settings.http_timeout_ms))
                .build()?,
            settings,
        })
    }

    pub async fn has_updates(&self, engine: &Engine) -> Result<bool> {
        #[cfg(feature = "tracing")]
        tracing::info!("Check updates");
        if engine.source_etag.is_empty() {
            #[cfg(feature = "tracing")]
            tracing::info!("Engine hasn't source ETAGs");
            return Ok(true);
        }

        let mut requests = vec![self.get_etag(self.settings.cities.url)];
        let mut results = vec!["cities"];
        if let Some(item) = &self.settings.names {
            requests.push(self.get_etag(item.url));
            results.push("names");
        }
        if let Some(url) = self.settings.countries_url {
            requests.push(self.get_etag(url));
            results.push("countries");
        }
        if let Some(url) = self.settings.admin1_codes_url {
            requests.push(self.get_etag(url));
            results.push("admin1_codes");
        }
        let responses = futures::future::join_all(requests).await;
        let results: HashMap<_, _> = results.into_iter().zip(responses.into_iter()).collect();

        for (entry, etag) in results.into_iter() {
            let current_etag = engine
                .source_etag
                .get(entry)
                .map(AsRef::as_ref)
                .unwrap_or("");
            let new_etag = etag?;
            if current_etag != new_etag {
                #[cfg(feature = "tracing")]
                tracing::info!("New version of {entry}");
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub async fn get_etag(&self, url: &str) -> Result<String> {
        let response = self.http_client.head(url).send().await?;
        #[cfg(feature = "tracing")]
        tracing::info!("Try HEAD {url}");

        Ok(response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_default())
    }

    pub async fn fetch(&self, url: &str, filename: Option<&str>) -> Result<(String, Vec<u8>)> {
        let response = self.http_client.get(url).send().await?;
        #[cfg(feature = "tracing")]
        tracing::info!("Try GET {url}");

        if !response.status().is_success() {
            anyhow::bail!("GET {url} return status {}", response.status())
        }

        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_default();

        let content = response.bytes().await?.to_vec();
        #[cfg(feature = "tracing")]
        tracing::info!("Downloaded {url} size: {}", content.len());

        let content = if let Some(filename) = filename {
            #[cfg(feature = "tracing")]
            tracing::info!("Unzip {filename}");
            let cursor = Cursor::new(content);
            let mut archive = zip::read::ZipArchive::new(cursor)?;
            let file = archive
                .by_name(filename)
                .map_err(|e| anyhow::anyhow!("On get file {filename} from archive: {e}"))?;
            file.bytes().collect::<std::io::Result<Vec<_>>>()?
        } else {
            content
        };

        Ok((etag, content))
    }

    pub async fn build(self) -> Result<Engine> {
        let mut requests = vec![self.fetch(
            self.settings.cities.url,
            Some(self.settings.cities.filename),
        )];
        let mut results = vec!["cities"];
        if let Some(item) = &self.settings.names {
            requests.push(self.fetch(item.url, Some(item.filename)));
            results.push("names");
        }
        if let Some(url) = self.settings.countries_url {
            requests.push(self.fetch(url, None));
            results.push("countries");
        }
        if let Some(url) = self.settings.admin1_codes_url {
            requests.push(self.fetch(url, None));
            results.push("admin1_codes");
        }
        if let Some(url) = self.settings.admin2_codes_url {
            requests.push(self.fetch(url, None));
            results.push("admin2_codes");
        }
        let responses = futures::future::join_all(requests).await;
        let mut results: HashMap<_, _> = results.into_iter().zip(responses.into_iter()).collect();

        let source_etag = results
            .iter()
            .filter_map(|(k, v)| {
                let Ok((etag, _)) = v else {
                return None
            };
                Some(((*k).to_string(), etag.to_string()))
            })
            .collect();

        #[cfg(feature = "tracing")]
        tracing::info!("Try to build index...");

        Engine::new_from_files_content(
            SourceFileContentOptions {
                cities: String::from_utf8(
                    results
                        .remove(&"cities")
                        .ok_or_else(|| anyhow::anyhow!("Cities file required"))?
                        .map_err(|e| anyhow::anyhow!("On fetch cities file: {e}"))?
                        .1, // .ok_or_else(|| anyhow::anyhow!("Cities file required"))?,
                )?,
                names: if let Some(c) = results.remove(&"names") {
                    Some(String::from_utf8(c?.1)?)
                } else {
                    None
                },
                countries: if let Some(c) = results.remove(&"countries") {
                    Some(String::from_utf8(c?.1)?)
                } else {
                    None
                },
                admin1_codes: if let Some(c) = results.remove(&"admin1_codes") {
                    Some(String::from_utf8(c?.1)?)
                } else {
                    None
                },
                admin2_codes: if let Some(c) = results.remove(&"admin2_codes") {
                    Some(String::from_utf8(c?.1)?)
                } else {
                    None
                },
                filter_languages: self.settings.filter_languages,
            },
            source_etag,
        )
        .map_err(|e| anyhow::anyhow!("Failed to build index: {e}"))
    }
}
