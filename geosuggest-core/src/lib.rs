#![doc = include_str!("../README.md")]
use std::collections::HashMap;

use kiddo::{self, SquaredEuclidean};

use rayon::prelude::*;
use rkyv::collections::swiss_table::ArchivedHashMap;
use rkyv::rend::{f32_le, u32_le};
use rkyv::string::ArchivedString;
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
    ArchivedAdminDivision, ArchivedCitiesRecord, ArchivedCountry, ArchivedCountryRecord,
    ArchivedIndexData, IndexData, NO_TABLE_INDEX,
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

#[derive(
    Debug, Default, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize,
)]
pub struct EngineSourceMetadata {
    pub cities: String,
    pub names: Option<String>,
    pub countries: Option<String>,
    pub admin1_codes: Option<String>,
    pub admin2_codes: Option<String>,
    pub filter_languages: Vec<String>,
    pub etag: HashMap<String, String>,
}

#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize)]
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
    /// Archived index layout version (`index::INDEX_FORMAT_VERSION`);
    /// `Storage::load` rejects anything else
    pub index_format_version: u32,
}

impl Default for EngineMetadata {
    fn default() -> Self {
        Self {
            created_at: std::time::SystemTime::now(),
            geosuggest_version: env!("CARGO_PKG_VERSION").to_owned(),
            source: EngineSourceMetadata::default(),
            extra: HashMap::default(),
            index_format_version: index::INDEX_FORMAT_VERSION,
        }
    }
}

pub struct EngineData {
    pub data: rkyv::util::AlignedVec<128>,
    pub metadata: Option<EngineMetadata>,
    #[cfg(feature = "geoip2")]
    pub geoip2: Option<Vec<u8>>,
}

impl EngineData {
    #[cfg(feature = "geoip2")]
    pub fn load_geoip2<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.geoip2 = std::fs::read(path)?.into();
        Ok(())
    }

    pub fn as_engine(&self) -> Result<Engine<'_>, Box<dyn std::error::Error>> {
        Ok(Engine {
            data: rkyv::access::<_, rkyv::rancor::Error>(&self.data)?,
            #[cfg(feature = "geoip2")]
            geoip2: if let Some(geoip2) = &self.geoip2 {
                Reader::<City>::from_bytes(geoip2)
                    .map_err(|e| format!("Geoip2 error: {e:?}"))?
                    .into()
            } else {
                None
            },
        })
    }
}

pub struct Engine<'a> {
    pub data: &'a ArchivedIndexData,
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

        // Ranked candidate. Descending order: higher score first; within
        // EPSILON, higher population first; exact ties by id so the order
        // stays deterministic (no order was ever promised between them).
        #[derive(Clone, Copy)]
        struct Cand {
            score: f32,
            pop: u32,
            id: u32,
        }
        fn rank_desc(a: &Cand, b: &Cand) -> std::cmp::Ordering {
            if (a.score - b.score).abs() < f32::EPSILON {
                b.pop.cmp(&a.pop).then_with(|| a.id.cmp(&b.id))
            } else {
                // same shape as the old comparator: higher score first
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        }

        // Highest-ranked `limit` distinct ids, each with its best score.
        // Matches the old collect-sort-unique-take result, except for the
        // order between exact cross-city ties (which was unspecified).
        fn select_top(mut cands: Vec<Cand>, limit: usize) -> Vec<Cand> {
            cands.sort_unstable_by(rank_desc);
            // linear dedup: `limit` is tiny (10 by default), so a scan
            // beats hashing here and needs no custom hasher
            let mut seen: Vec<u32> = Vec::new();
            let mut out = Vec::with_capacity(limit.min(cands.len()));
            for c in cands {
                if out.len() == limit {
                    break;
                }
                if !seen.contains(&c.id) {
                    seen.push(c.id);
                    out.push(c);
                }
            }
            out
        }

        let allowed: Option<Vec<u32>> = countries.map(|countries| {
            countries
                .iter()
                .filter_map(|code| {
                    self.data
                        .country_info_by_code
                        .get(code.as_ref())
                        .map(|c| c.info.geonameid.to_native())
                })
                .collect::<Vec<_>>()
        });

        // Scan in chunks. Few matches stay in a plain vector, exactly like
        // the old full collect; past 64K sightings the accumulator compacts
        // to the top `limit` distinct ids. Either way memory stays far below
        // the old unbounded collect, and a dropped id always re-enters
        // through its later sightings with a higher score, so the surviving
        // set is exact.
        const CHUNK: usize = 65_536;
        const COMPACT_AT: usize = 65_536;
        let mut acc: Vec<Cand> = Vec::new();

        // No compaction on the last chunk: the final selection sorts it once.
        let chunks = self.data.entries.as_slice().chunks(CHUNK);
        let last_chunk = chunks.len().saturating_sub(1);
        for (n, chunk) in chunks.enumerate() {
            let local: Vec<Cand> = chunk
                .par_iter()
                .filter(|item| match &allowed {
                    Some(ids) => ids.contains(&item.country_id.to_native()),
                    None => true,
                })
                .filter_map(|item| {
                    let score = if item.value.starts_with(&normalized_pattern) {
                        1.0
                    } else {
                        jaro_winkler(&item.value, &normalized_pattern) as f32
                    };
                    if score >= min_score {
                        self.data.geonames.get(&item.id).map(|city| Cand {
                            score,
                            pop: city.population.to_native(),
                            id: item.id.to_native(),
                        })
                    } else {
                        None
                    }
                })
                .collect();
            acc.extend(local);
            if acc.len() >= COMPACT_AT && n < last_chunk {
                acc = select_top(acc, limit);
            }
        }

        select_top(acc, limit)
            .into_iter()
            .filter_map(|c| self.data.geonames.get(&u32_le::from_native(c.id)))
            .collect()
    }

    /// Find the nearest cities by coordinates.
    ///
    /// Optional: score results by `k` as `distance - k * city.population` and sort by score.
    ///
    /// Optional: prefilter by countries. Filtered queries fetch nearest in
    /// growing rounds instead of the whole index at once; building an index
    /// for concrete countries is still faster if the filter is always the
    /// same. A filter matching almost nothing still ends with one
    /// full-index fetch.
    pub fn reverse<T: AsRef<str>>(
        &self,
        loc: (f32, f32),
        limit: usize,
        k: Option<f32>,
        countries: Option<&[T]>,
    ) -> Option<Vec<ArchivedReverseItem<'_>>> {
        if limit == 0 {
            return None;
        }

        // resolve codes to shared-table indices once; comparing integers per
        // candidate is cheaper than comparing strings
        let allowed: Option<Vec<u32>> = countries.as_ref().map(|codes| {
            codes
                .iter()
                .filter_map(|code| {
                    self.data
                        .countries
                        .iter()
                        .position(|c| c.code.as_str() == code.as_ref())
                        .map(|pos| pos as u32)
                })
                .collect()
        });

        let total = self.data.geonames.len();
        // without a filter the single query below behaves exactly like
        // before; with a filter an empty index keeps returning None
        let mut round = match &allowed {
            Some(_) => {
                if total == 0 {
                    return None;
                }
                limit.min(total)
            }
            None => limit,
        };

        // With a country filter, fetch nearest in growing rounds instead of
        // the whole index at once. Stopping at the first round that covers
        // `limit` in-country cities sees the same candidates, so results are
        // identical. Typical filters stop after one or two small rounds; a
        // filter matching nothing still ends with one full-index fetch,
        // exactly like the old code always did.
        let mut results = loop {
            let nearest_limit = std::num::NonZero::new(round)?;
            let mut results = self
                .data
                .tree
                .query(&[loc.0, loc.1])
                .nearest_n::<SquaredEuclidean<f32>>(nearest_limit)
                .execute();
            let enough = match &allowed {
                Some(ids) => {
                    results
                        .iter_mut()
                        .filter(|nearest| {
                            self.data
                                .tree_index_to_geonameid
                                .get(nearest.item as usize)
                                .and_then(|geonameid| self.data.geonames.get(geonameid))
                                .map(|city| ids.contains(&city.country_idx.to_native()))
                                .unwrap_or(false)
                        })
                        .count()
                        >= limit
                }
                None => true,
            };
            if enough || round >= total {
                break results;
            }
            round = round.saturating_mul(2).min(total);
        };

        let mut i1;
        let mut i2;

        let items = &mut results;

        let items: &mut dyn Iterator<Item = (_, &ArchivedCitiesRecord)> =
            if allowed.is_some() {
                i1 = items.iter_mut().filter_map(|nearest| {
                    let geonameid = self
                        .data
                        .tree_index_to_geonameid
                        .get(nearest.item as usize)?;
                    let city = self.data.geonames.get(geonameid)?;
                    if allowed
                        .as_ref()
                        .map(|ids| ids.contains(&city.country_idx.to_native()))
                        .unwrap_or(false)
                    {
                        Some((nearest, city))
                    } else {
                        None
                    }
                });
                &mut i1
            } else {
                i2 = items.iter_mut().filter_map(|nearest| {
                    let geonameid = self
                        .data
                        .tree_index_to_geonameid
                        .get(nearest.item as usize)?;
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

    /// Country of a city via the shared countries table.
    pub fn city_country(&self, city: &ArchivedCitiesRecord) -> Option<&ArchivedCountry> {
        if city.country_idx.to_native() == NO_TABLE_INDEX {
            None
        } else {
            self.data.countries.get(city.country_idx.to_native() as usize)
        }
    }

    /// Admin1 division of a city via the shared divisions table.
    pub fn city_admin1(&self, city: &ArchivedCitiesRecord) -> Option<&ArchivedAdminDivision> {
        if city.admin1_idx.to_native() == NO_TABLE_INDEX {
            None
        } else {
            self.data
                .admin1_divisions
                .get(city.admin1_idx.to_native() as usize)
        }
    }

    /// Admin2 division of a city via the shared divisions table.
    pub fn city_admin2(&self, city: &ArchivedCitiesRecord) -> Option<&ArchivedAdminDivision> {
        if city.admin2_idx.to_native() == NO_TABLE_INDEX {
            None
        } else {
            self.data
                .admin2_divisions
                .get(city.admin2_idx.to_native() as usize)
        }
    }

    /// Timezone of a city via the shared timezones table.
    pub fn city_timezone(&self, city: &ArchivedCitiesRecord) -> &str {
        self.data
            .timezones
            .get(city.timezone_idx.to_native() as usize)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Country name translations by iso 2-letter country code.
    ///
    /// Shared table; replaces the former per-city copy.
    pub fn country_names(
        &self,
        country_code: &str,
    ) -> Option<&ArchivedHashMap<ArchivedString, ArchivedString>> {
        self.data.country_info_by_code.get(country_code)?.names.as_ref()
    }

    /// Admin1 division name translations by division geonameid.
    pub fn admin1_names(
        &self,
        division_id: u32,
    ) -> Option<&ArchivedHashMap<ArchivedString, ArchivedString>> {
        self.data
            .admin1_names
            .get(&u32_le::from_native(division_id))
    }

    /// Admin2 division name translations by division geonameid.
    pub fn admin2_names(
        &self,
        division_id: u32,
    ) -> Option<&ArchivedHashMap<ArchivedString, ArchivedString>> {
        self.data
            .admin2_names
            .get(&u32_le::from_native(division_id))
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

impl TryFrom<IndexData> for EngineData {
    type Error = rkyv::rancor::Error;
    fn try_from(data: IndexData) -> Result<EngineData, Self::Error> {
        let mut bytes = rkyv::api::high::to_bytes_in::<_, rkyv::rancor::Error>(
            &data,
            rkyv::util::AlignedVec::<128>::new(),
        )?;
        // the serializer grows by doubling; drop the slack before long-term storage
        bytes.shrink_to_fit();
        Ok(EngineData {
            data: bytes,
            metadata: None,
            #[cfg(feature = "geoip2")]
            geoip2: None,
        })
    }
}

impl TryFrom<rkyv::util::AlignedVec<128>> for EngineData {
    type Error = rkyv::rancor::Error;
    fn try_from(bytes: rkyv::util::AlignedVec<128>) -> Result<EngineData, Self::Error> {
        Ok(EngineData {
            data: bytes,
            metadata: None,
            #[cfg(feature = "geoip2")]
            geoip2: None,
        })
    }
}
