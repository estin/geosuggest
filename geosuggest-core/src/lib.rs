#![doc = include_str!("../README.md")]
use std::collections::HashMap;

use itertools::Itertools;

use kiddo::{self, SquaredEuclidean};

use kiddo::immutable::float::kdtree::ImmutableKdTree;

use rayon::prelude::*;
use strsim::jaro_winkler;

#[cfg(feature = "geoip2")]
use std::net::IpAddr;

#[cfg(feature = "geoip2")]
use geoip2::{City, Reader};

#[cfg(feature = "oaph")]
use oaph::schemars::{self, JsonSchema};

pub mod index;
pub mod storage;

use index::{CitiesRecord, CountryRecord, Entry, IndexData};

#[cfg_attr(feature = "oaph", derive(JsonSchema))]
#[derive(Debug, serde::Serialize)]
pub struct ReverseItem<'a> {
    pub city: &'a index::CitiesRecord,
    pub distance: f32,
    pub score: f32,
}

#[derive(Debug, Default, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
pub struct EngineSourceMetadata {
    pub cities: String,
    pub names: Option<String>,
    pub countries: Option<String>,
    pub admin1_codes: Option<String>,
    pub admin2_codes: Option<String>,
    pub filter_languages: Vec<String>,
    pub etag: HashMap<String, String>,
}

#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
pub struct EngineMetadata {
    /// Index was built on version
    pub geosuggest_version: String,
    /// Creation time
    #[rkyv(with = rkyv::with::AsUnixTime)]
    pub created_at: std::time::SystemTime,
    /// Sources metadata
    pub source: EngineSourceMetadata,
    /// Custom metadata info
    pub extra: HashMap<String, String>,
}

impl Default for EngineMetadata {
    fn default() -> Self {
        Self {
            created_at: std::time::SystemTime::now(),
            geosuggest_version: env!("CARGO_PKG_VERSION").to_owned(),
            source: EngineSourceMetadata::default(),
            extra: HashMap::default(),
        }
    }
}

// #[derive(Clone, rkyv::Serialize, rkyv::Archive)]
pub struct Engine {
    pub data: IndexData,
    pub metadata: Option<EngineMetadata>,

    tree_index_to_geonameid: HashMap<usize, u32>,
    tree: ImmutableKdTree<f32, u32, 2, 32>,
    #[cfg(feature = "geoip2")]
    geoip2_reader: Option<(&'static Vec<u8>, &'static Reader<'static, City<'static>>)>,
}

impl Engine {
    pub fn get(&self, id: &u32) -> Option<&CitiesRecord> {
        self.data.geonames.get(id)
    }

    pub fn capital(&self, country_code: &str) -> Option<&CitiesRecord> {
        if let Some(city_id) = self.data.capitals.get(&country_code.to_uppercase()) {
            self.get(city_id)
        } else {
            None
        }
    }

    /// Suggest cities by pattern (multilang).
    ///
    /// Optional: filter by Jaro–Winkler distance via min_score
    ///
    /// Optional: prefilter by countries
    pub fn suggest<T: AsRef<str>>(
        &self,
        pattern: &str,
        limit: usize,
        min_score: Option<f32>,
        countries: Option<&[T]>,
    ) -> Vec<&CitiesRecord> {
        if limit == 0 {
            return Vec::new();
        }

        let min_score = min_score.unwrap_or(0.8);
        let normalized_pattern = pattern.to_lowercase();

        let filter_by_pattern = |item: &Entry| -> Option<(&CitiesRecord, f32)> {
            let score = if item.value.starts_with(&normalized_pattern) {
                1.0
            } else {
                jaro_winkler(&item.value, &normalized_pattern) as f32
            };
            if score >= min_score {
                self.data.geonames.get(&item.id).map(|city| (city, score))
            } else {
                None
            }
        };

        let mut result: Vec<(&CitiesRecord, f32)> = match &countries {
            Some(countries) => {
                let country_ids = countries
                    .iter()
                    .filter_map(|code| {
                        self.data
                            .country_info_by_code
                            .get(&code.as_ref().to_uppercase())
                            .map(|c| &c.info.geonameid)
                    })
                    .collect::<Vec<&u32>>();
                self.data
                    .entries
                    .par_iter()
                    .filter(|item| {
                        if let Some(country_id) = &item.country_id {
                            country_ids.contains(&country_id)
                        } else {
                            false
                        }
                    })
                    .filter_map(filter_by_pattern)
                    .collect()
            }
            None => self
                .data
                .entries
                .par_iter()
                .filter_map(filter_by_pattern)
                .collect(),
        };

        // sort by score desc, population desc
        result.sort_unstable_by(|lhs, rhs| {
            if (lhs.1 - rhs.1).abs() < f32::EPSILON {
                rhs.0
                    .population
                    .partial_cmp(&lhs.0.population)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                rhs.1
                    .partial_cmp(&lhs.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        result
            .iter()
            .unique_by(|item| item.0.id)
            .take(limit)
            .map(|item| item.0)
            .collect::<Vec<&CitiesRecord>>()
    }

    /// Find the nearest cities by coordinates.
    ///
    /// Optional: score results by `k` as `distance - k * city.population` and sort by score.
    ///
    /// Optional: prefilter by countries. It's a very expensive case; consider building an index for concrete countries and not applying this filter at all.
    pub fn reverse<T: AsRef<str>>(
        &self,
        loc: (f32, f32),
        limit: usize,
        k: Option<f32>,
        countries: Option<&[T]>,
    ) -> Option<Vec<ReverseItem>> {
        if limit == 0 {
            return None;
        }

        let nearest_limit = std::num::NonZero::new(if countries.is_some() {
            // ugly hack try to fetch nearest cities in requested countries
            // much better is to build index for concrete countries
            self.data.geonames.len()
        } else {
            limit
        })?;

        let mut i1;
        let mut i2;

        let items = &mut self
            .tree
            .nearest_n::<SquaredEuclidean>(&[loc.0, loc.1], nearest_limit);

        let items: &mut dyn Iterator<Item = (_, &CitiesRecord)> = if let Some(countries) = countries
        {
            // normalize
            let countries = countries
                .iter()
                .map(|code| code.as_ref().to_uppercase())
                .collect::<Vec<_>>();

            i1 = items.iter_mut().filter_map(move |nearest| {
                let geonameid = self.tree_index_to_geonameid.get(&(nearest.item as usize))?;
                let city = self.data.geonames.get(geonameid)?;
                let country = city.country.as_ref()?;
                if countries.contains(&country.code) {
                    Some((nearest, city))
                } else {
                    None
                }
            });
            &mut i1
        } else {
            i2 = items.iter_mut().filter_map(|nearest| {
                let geonameid = self.tree_index_to_geonameid.get(&(nearest.item as usize))?;
                let city = self.data.geonames.get(geonameid)?;
                Some((nearest, city))
            });
            &mut i2
        };

        if let Some(k) = k {
            let mut points = items
                .map(|item| {
                    (
                        item.0.distance,
                        item.0.distance - k * item.1.population as f32,
                        item.1,
                    )
                })
                .take(limit)
                .collect::<Vec<_>>();

            points.sort_unstable_by(|a, b| {
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            });

            Some(
                points
                    .iter()
                    .map(|p| ReverseItem {
                        distance: p.0,
                        score: p.1,
                        city: p.2,
                    })
                    .collect(),
            )
        } else {
            Some(
                items
                    .map(|item| ReverseItem {
                        distance: item.0.distance,
                        score: item.0.distance,
                        city: item.1,
                    })
                    .take(limit)
                    .collect(),
            )
        }
    }

    /// Get country info by iso 2-letter country code.
    pub fn country_info(&self, country_code: &str) -> Option<&CountryRecord> {
        self.data
            .country_info_by_code
            .get(&country_code.to_uppercase())
    }

    // TODO slim mmdb size, we are needs only geonameid
    /// **unsafe** method to initialize geoip2 buffer and reader
    #[cfg(feature = "geoip2")]
    pub fn load_geoip2<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // consume and release memory of previously leaked buffer and reader
        if let Some((b, r)) = self.geoip2_reader.take() {
            // make Box<T> from raw pointer to drop it
            let b = b as *const Vec<u8>;
            let _ = unsafe { Box::from_raw(b as *mut Vec<u8>) };
            let r = r as *const Reader<'static, City<'static>>;
            let _ = unsafe { Box::from_raw(r as *mut Reader<'static, City<'static>>) };
        }

        // leak geoip buffer and reader with reference to buffer
        let buffer = std::fs::read(path)?;
        let buffer: &'static Vec<u8> = Box::leak(Box::new(buffer));
        let reader = Reader::<City>::from_bytes(buffer).map_err(GeoIP2Error)?;
        let reader: &'static Reader<City> = Box::leak(Box::new(reader));

        self.geoip2_reader = Some((buffer, reader));

        Ok(())
    }

    #[cfg(feature = "geoip2")]
    pub fn geoip2_lookup(&self, addr: IpAddr) -> Option<&CitiesRecord> {
        match self.geoip2_reader.as_ref() {
            Some((_, reader)) => {
                let result = reader.lookup(addr).ok()?;
                let city = result.city?;
                let id = city.geoname_id?;
                self.data.geonames.get(&id)
            }
            None => {
                #[cfg(feature = "tracing")]
                tracing::warn!("Geoip2 reader is't configured!");
                None
            }
        }
    }
}

#[cfg(feature = "geoip2")]
struct GeoIP2Error(geoip2::Error);

#[cfg(feature = "geoip2")]
impl std::error::Error for GeoIP2Error {}

#[cfg(feature = "geoip2")]
impl std::fmt::Debug for GeoIP2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[cfg(feature = "geoip2")]
impl std::fmt::Display for GeoIP2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GeoIP2 Error {:?}", self.0)
    }
}

impl From<IndexData> for Engine {
    fn from(data: IndexData) -> Engine {
        let mut items = data
            .geonames
            .values()
            .map(|record| (record.id, [record.latitude, record.longitude]))
            .collect::<Vec<_>>();

        items.sort_unstable_by_key(|item| item.0);
        items.dedup_by_key(|item| item.0);

        let tree_index_to_geonameid = HashMap::from_iter(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (index, item.0)),
        );
        let tree = ImmutableKdTree::new_from_slice(
            items
                .into_iter()
                .map(|item| item.1)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        Engine {
            data,
            metadata: None,
            tree_index_to_geonameid,
            tree,
            #[cfg(feature = "geoip2")]
            geoip2_reader: None,
        }
    }
}
