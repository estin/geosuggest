#![doc = include_str!("../README.md")]
use std::collections::HashMap;

use itertools::Itertools;

use kiddo::{self, SquaredEuclidean};

use kiddo::immutable::float::kdtree::ImmutableKdTree;

use rayon::prelude::*;
use rkyv::rend::{f32_le, u32_le};
use strsim::jaro_winkler;

#[cfg(feature = "geoip2")]
use std::net::IpAddr;

#[cfg(feature = "geoip2")]
use geoip2::{City, Reader};

#[cfg(feature = "oaph")]
use oaph::schemars::{self, JsonSchema};

pub mod index;
pub mod storage;

use index::{
    ArchivedCitiesRecord, ArchivedCountryRecord, ArchivedEntry, ArchivedIndexData, IndexData,
};

#[cfg_attr(feature = "oaph", derive(JsonSchema))]
#[derive(Debug, serde::Serialize)]
pub struct ReverseItem<'a> {
    pub city: &'a index::CitiesRecord,
    pub distance: f32,
    pub score: f32,
}

#[derive(Debug, serde::Serialize)]
pub struct ArchivedReverseItem<'a> {
    pub city: &'a index::ArchivedCitiesRecord,
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

pub struct EngineData {
    pub data: rkyv::util::AlignedVec,
    pub metadata: Option<EngineMetadata>,

    #[cfg(feature = "geoip2")]
    pub geoip2: Option<Vec<u8>>,
    tree_index_to_geonameid: HashMap<usize, u32_le>,
    tree: ImmutableKdTree<f32, u32, 2, 32>,
}

impl EngineData {
    #[cfg(feature = "geoip2")]
    pub fn load_geoip2<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // consume and release memory of previously leaked buffer and reader
        self.geoip2 = std::fs::read(path)?.into();

        Ok(())
    }

    pub fn as_engine(&self) -> Result<Engine, rkyv::rancor::Error> {
        Ok(Engine {
            data: rkyv::access(&self.data)?,
            tree_index_to_geonameid: &self.tree_index_to_geonameid,
            tree: &self.tree,
            #[cfg(feature = "geoip2")]
            geoip2: if let Some(geoip2) = &self.geoip2 {
                Reader::<City>::from_bytes(geoip2)
                    .map_err(GeoIP2Error)
                    .unwrap()
                    .into()
            } else {
                None
            },
        })
    }
}

pub struct Engine<'a> {
    pub data: &'a ArchivedIndexData,
    tree_index_to_geonameid: &'a HashMap<usize, u32_le>,
    tree: &'a ImmutableKdTree<f32, u32, 2, 32>,
    #[cfg(feature = "geoip2")]
    geoip2: Option<Reader<'a, City<'a>>>,
}

impl Engine<'_> {
    pub fn get(&self, id: &u32) -> Option<&ArchivedCitiesRecord> {
        self.data.geonames.get(&u32_le::from_native(*id))
    }

    /// Get capital by uppercase country code
    pub fn capital(&self, country_code: &str) -> Option<&ArchivedCitiesRecord> {
        if let Some(city_id) = self.data.capitals.get(country_code) {
            self.data.geonames.get(city_id)
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
    ) -> Vec<&ArchivedCitiesRecord> {
        if limit == 0 {
            return Vec::new();
        }

        let min_score = min_score.unwrap_or(0.8);
        let normalized_pattern = pattern.to_lowercase();

        let filter_by_pattern = |item: &ArchivedEntry| -> Option<(&ArchivedCitiesRecord, f32)> {
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

        let mut result: Vec<(&ArchivedCitiesRecord, f32)> = match &countries {
            Some(countries) => {
                let country_ids = countries
                    .iter()
                    .filter_map(|code| {
                        self.data
                            .country_info_by_code
                            .get(code.as_ref())
                            .map(|c| &c.info.geonameid)
                    })
                    .collect::<Vec<_>>();
                self.data
                    .entries
                    .par_iter()
                    .filter(|item| {
                        item.country_id
                            .as_ref()
                            .map(|id| country_ids.contains(&id))
                            .unwrap_or_default()
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
            .collect::<Vec<_>>()
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
    ) -> Option<Vec<ArchivedReverseItem>> {
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

        let items: &mut dyn Iterator<Item = (_, &ArchivedCitiesRecord)> =
            if let Some(countries) = countries {
                // normalize
                let countries = countries
                    .iter()
                    .map(|code| code.as_ref())
                    .collect::<Vec<_>>();

                i1 = items.iter_mut().filter_map(move |nearest| {
                    let geonameid = self.tree_index_to_geonameid.get(&(nearest.item as usize))?;
                    let city = self.data.geonames.get(geonameid)?;
                    let country = city.country.as_ref()?;
                    if countries.contains(&country.code.as_str()) {
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

        if let Some(k) = k.map(f32_le::from_native) {
            let mut points = items
                .map(|item| {
                    (
                        item.0.distance,
                        item.0.distance - k * (item.1.population.to_native() as f32),
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
                    .map(|p| ArchivedReverseItem {
                        distance: p.0,
                        score: p.1,
                        city: p.2,
                    })
                    .collect(),
            )
        } else {
            Some(
                items
                    .map(|item| ArchivedReverseItem {
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
    pub fn country_info(&self, country_code: &str) -> Option<&ArchivedCountryRecord> {
        self.data.country_info_by_code.get(country_code)
    }

    #[cfg(feature = "geoip2")]
    pub fn geoip2_lookup(&self, addr: IpAddr) -> Option<&ArchivedCitiesRecord> {
        match self.geoip2.as_ref() {
            Some(reader) => {
                let result = reader.lookup(addr).ok()?;
                let city = result.city?;
                let id = city.geoname_id?;
                self.data.geonames.get(&u32_le::from_native(id))
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

impl TryFrom<IndexData> for EngineData {
    type Error = rkyv::rancor::Error;
    fn try_from(data: IndexData) -> Result<EngineData, Self::Error> {
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
                .map(|(index, item)| (index, u32_le::from_native(item.0))),
        );
        let tree = ImmutableKdTree::new_from_slice(
            items
                .into_iter()
                .map(|item| item.1)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        Ok(EngineData {
            data: rkyv::to_bytes(&data)?,
            metadata: None,
            tree_index_to_geonameid,
            tree,
            #[cfg(feature = "geoip2")]
            geoip2: None,
        })
    }
}

impl TryFrom<rkyv::util::AlignedVec> for EngineData {
    type Error = rkyv::rancor::Error;
    fn try_from(bytes: rkyv::util::AlignedVec) -> Result<EngineData, Self::Error> {
        let data = rkyv::access::<ArchivedIndexData, rkyv::rancor::Error>(&bytes[..])?;

        let mut items = data
            .geonames
            .values()
            .map(|record| {
                (
                    record.id.to_native(),
                    [record.latitude.to_native(), record.longitude.to_native()],
                )
            })
            .collect::<Vec<_>>();

        items.sort_unstable_by_key(|item| item.0);
        items.dedup_by_key(|item| item.0);

        let tree_index_to_geonameid = HashMap::from_iter(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (index, u32_le::from_native(item.0))),
        );
        let tree = ImmutableKdTree::new_from_slice(
            items
                .into_iter()
                .map(|item| item.1)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        Ok(EngineData {
            data: bytes,
            metadata: None,
            tree_index_to_geonameid,
            tree,
            #[cfg(feature = "geoip2")]
            geoip2: None,
        })
    }
}
