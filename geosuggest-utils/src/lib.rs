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

    pub async fn fetch(&self, url: &str, filename: Option<&str>) -> Result<Vec<u8>> {
        let response = self.http_client.get(url).send().await?;
        tracing::info!("Try GET {url}");

        if !response.status().is_success() {
            anyhow::bail!("GET {url} return status {}", response.status())
        }

        let content = response.bytes().await?.to_vec();
        tracing::info!("Downloaded {url} size: {}", content.len());

        if let Some(filename) = filename {
            let cursor = Cursor::new(content);
            let mut archive = zip::read::ZipArchive::new(cursor)?;
            let file = archive
                .by_name(filename)
                .map_err(|e| anyhow::anyhow!("On get file {filename} from archive: {e}"))?;
            Ok(file.bytes().collect::<std::io::Result<Vec<_>>>()?)
        } else {
            Ok(content)
        }
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
        }
        if let Some(url) = self.settings.admin1_codes_url {
            requests.push(self.fetch(url, None));
            results.push("admin1_codes");
        }
        let responses = ntex::util::join_all(requests).await;
        let mut results: HashMap<_, _> = results.into_iter().zip(responses.into_iter()).collect();

        Engine::new_from_files_content(SourceFileContentOptions {
            cities: String::from_utf8(
                results
                    .remove(&"cities")
                    .ok_or_else(|| anyhow::anyhow!("Cities file required"))?
                    .map_err(|e| anyhow::anyhow!("On fetch cities file: {e}"))?, // .ok_or_else(|| anyhow::anyhow!("Cities file required"))?,
            )?,
            names: if let Some(c) = results.remove(&"names") {
                Some(String::from_utf8(c?)?)
            } else {
                None
            },
            countries: if let Some(c) = results.remove(&"admin1_codes") {
                Some(String::from_utf8(c?)?)
            } else {
                None
            },
            admin1_codes: if let Some(c) = results.remove(&"admin1_codes") {
                Some(String::from_utf8(c?)?)
            } else {
                None
            },
            filter_languages: self.settings.filter_languages,
        })
        .map_err(|e| anyhow::anyhow!("Failed to build index: {e}"))
    }
}
