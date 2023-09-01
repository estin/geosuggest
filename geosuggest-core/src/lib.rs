use std::collections::{HashMap, HashSet};
use std::error::Error;

#[cfg(feature = "tracing")]
use std::time::Instant;

use itertools::Itertools;

// use kiddo::{self, distance::squared_euclidean, KdTree};
use kiddo::{
    self,
    float::{distance::squared_euclidean, kdtree::KdTree},
};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strsim::jaro_winkler;

#[cfg(feature = "geoip2_support")]
use std::net::IpAddr;

#[cfg(feature = "geoip2_support")]
use geoip2::{City, Reader};

#[cfg(feature = "oaph_support")]
use oaph::schemars::{self, JsonSchema};

type CountryFilterFn = dyn Fn(&CitiesRecord) -> Option<()>;

pub struct SourceFileOptions<'a, P: AsRef<std::path::Path>> {
    pub cities: P,
    pub names: Option<P>,
    pub countries: Option<P>,
    pub admin1_codes: Option<P>,
    pub admin2_codes: Option<P>,
    pub filter_languages: Vec<&'a str>,
}

pub struct SourceFileContentOptions<'a> {
    pub cities: String,
    pub names: Option<String>,
    pub countries: Option<String>,
    pub admin1_codes: Option<String>,
    pub admin2_codes: Option<String>,
    pub filter_languages: Vec<&'a str>,
}

// code, name, name ascii, geonameid
#[derive(Debug, Deserialize)]
struct Admin1CodeRecordRaw {
    code: String,
    name: String,
    _asciiname: String,
    geonameid: u32,
}

// code, name, name ascii, geonameid
#[derive(Debug, Deserialize)]
struct Admin2CodeRecordRaw {
    code: String,
    name: String,
    _asciiname: String,
    geonameid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "oaph_support", derive(JsonSchema))]
pub struct AdminDivision {
    pub id: u32,
    pub code: String,
    pub name: String,
}

// The main 'geoname' table has the following fields :
// ---------------------------------------------------
// geonameid         : integer id of record in geonames database
// name              : name of geographical point (utf8) varchar(200)
// asciiname         : name of geographical point in plain ascii characters, varchar(200)
// alternatenames    : alternatenames, comma separated, ascii names automatically transliterated, convenience attribute from alternatename table, varchar(10000)
// latitude          : latitude in decimal degrees (wgs84)
// longitude         : longitude in decimal degrees (wgs84)
// feature class     : see http://www.geonames.org/export/codes.html, char(1)
// feature code      : see http://www.geonames.org/export/codes.html, varchar(10)
// country code      : ISO-3166 2-letter country code, 2 characters
// cc2               : alternate country codes, comma separated, ISO-3166 2-letter country code, 200 characters
// admin1 code       : fipscode (subject to change to iso code), see exceptions below, see file admin1Codes.txt for display names of this code; varchar(20)
// admin2 code       : code for the second administrative division, a county in the US, see file admin2Codes.txt; varchar(80)
// admin3 code       : code for third level administrative division, varchar(20)
// admin4 code       : code for fourth level administrative division, varchar(20)
// population        : bigint (8 byte int)
// elevation         : in meters, integer
// dem               : digital elevation model, srtm3 or gtopo30, average elevation of 3''x3'' (ca 90mx90m) or 30''x30'' (ca 900mx900m) area in meters, integer. srtm processed by cgiar/ciat.
// timezone          : the iana timezone id (see file timeZone.txt) varchar(40)
// modification date : date of last modification in yyyy-MM-dd format

#[derive(Debug, Deserialize)]
struct CitiesRecordRaw {
    geonameid: u32,
    name: String,
    asciiname: String,
    alternatenames: String,
    latitude: f32,
    longitude: f32,
    _feature_class: String,
    feature_code: String,
    country_code: String,
    _cc2: String,
    admin1_code: String,
    admin2_code: String,
    _admin3_code: String,
    _admin4_code: String,
    population: u32,
    _elevation: String,
    _dem: String,
    timezone: String,
    _modification_date: String,
}

// CounntryInfo
// http://download.geonames.org/export/dump/countryInfo.txt
// ISO	ISO3	ISO-Numeric	fips	Country	Capital	Area(in sq km)	Population	Continent	tld	CurrencyCode	CurrencyName	Phone	Postal Code Format	Postal Code Regex	Languages	geonameid	neighbours	EquivalentFipsCode
#[derive(Debug, Deserialize)]
struct CountryInfoRaw {
    iso: String,
    _iso3: String,
    _iso_numeric: String,
    _fips: String,
    name: String,
    _capital: String,
    _area: String,
    _population: u32,
    _continent: String,
    _tld: String,
    _currency_code: String,
    _currency_name: String,
    _phone: String,
    _postal_code_format: String,
    _postal_code_regex: String,
    _languages: String,
    geonameid: u32,
    _neighbours: String,
    _equivalent_fips_code: String,
}

// The table 'alternate names' :
// -----------------------------
// alternateNameId   : the id of this alternate name, int
// geonameid         : geonameId referring to id in table 'geoname', int
// isolanguage       : iso 639 language code 2- or 3-characters; 4-characters 'post' for postal codes and 'iata','icao' and faac for airport codes, fr_1793 for French Revolution names,  abbr for abbreviation, link to a website (mostly to wikipedia), wkdt for the wikidataid, varchar(7)
// alternate name    : alternate name or name variant, varchar(400)
// isPreferredName   : '1', if this alternate name is an official/preferred name
// isShortName       : '1', if this is a short name like 'California' for 'State of California'
// isColloquial      : '1', if this alternate name is a colloquial or slang term. Example: 'Big Apple' for 'New York'.
// isHistoric        : '1', if this alternate name is historic and was used in the past. Example 'Bombay' for 'Mumbai'.
// from		  : from period when the name was used
// to		  : to period when the name was used
#[derive(Debug, Deserialize)]
struct AlternateNamesRaw {
    _alternate_name_id: u32,
    geonameid: u32,
    isolanguage: String,
    alternate_name: String,
    is_prefered_name: String,
    is_short_name: String,
    is_colloquial: String,
    is_historic: String,
    _from: String,
    _to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "oaph_support", derive(JsonSchema))]
pub struct Country {
    pub id: u32,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "oaph_support", derive(JsonSchema))]
pub struct CitiesRecord {
    pub id: u32,
    pub name: String,
    pub latitude: f32,
    pub longitude: f32,
    pub country: Option<Country>,
    pub admin_division: Option<AdminDivision>,
    pub admin2_division: Option<AdminDivision>,
    pub timezone: String,
    pub names: Option<HashMap<String, String>>,
    pub country_names: Option<HashMap<String, String>>,
    pub admin1_names: Option<HashMap<String, String>>,
    pub admin2_names: Option<HashMap<String, String>>,
    pub population: u32,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "oaph_support", derive(JsonSchema))]
pub struct ReverseItem<'a> {
    pub city: &'a CitiesRecord,
    pub distance: f32,
    pub score: f32,
}

#[derive(Deserialize)]
struct EngineDump {
    source_etag: HashMap<String, String>,
    entries: Vec<Entry>,
    geonames: HashMap<u32, CitiesRecord>,
    capitals: HashMap<String, u32>,
    country_id_by_code: HashMap<String, u32>,
}

#[derive(Debug, Default)]
pub enum EngineDumpFormat {
    Json,
    #[default]
    Bincode,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    id: u32,                 // geoname id
    value: String,           // searchable value
    country_id: Option<u32>, // geoname country id
}

#[derive(Serialize)]
pub struct Engine {
    pub source_etag: HashMap<String, String>,
    entries: Vec<Entry>,
    geonames: HashMap<u32, CitiesRecord>,
    capitals: HashMap<String, u32>,
    country_id_by_code: HashMap<String, u32>,

    #[serde(skip_serializing)]
    tree: KdTree<f32, u32, 2, 32, u16>,

    #[cfg(feature = "geoip2_support")]
    #[serde(skip_serializing)]
    geoip2_reader: Option<(&'static Vec<u8>, &'static Reader<'static, City<'static>>)>,
}

pub fn skip_comment_lines(content: String) -> String {
    content.lines().filter(|l| !l.starts_with('#')).join("\n")
}

impl Engine {
    pub fn get(&self, id: &u32) -> Option<&CitiesRecord> {
        self.geonames.get(id)
    }

    pub fn capital(&self, country_code: &str) -> Option<&CitiesRecord> {
        if let Some(city_id) = self.capitals.get(&country_code.to_lowercase()) {
            self.get(city_id)
        } else {
            None
        }
    }

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
            if score > min_score {
                self.geonames.get(&item.id).map(|city| (city, score))
            } else {
                None
            }
        };

        let mut result: Vec<(&CitiesRecord, f32)> = match &countries {
            // prefilter by countries
            Some(countries) => {
                let country_ids = countries
                    .iter()
                    .filter_map(|code| self.country_id_by_code.get(&code.as_ref().to_uppercase()))
                    .collect::<Vec<&u32>>();
                self.entries
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
                .entries
                .par_iter()
                .filter_map(filter_by_pattern)
                .collect(),
        };

        // sort by score desc, population desc
        result.sort_by(|lhs, rhs| {
            if lhs.1 == rhs.1 {
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
        let k = k.unwrap_or(0.0);

        let filter_by_countries: Box<CountryFilterFn> = if let Some(countries) = countries {
            // normalize
            let countries = countries
                .iter()
                .map(|code| code.as_ref().to_uppercase())
                .collect::<Vec<_>>();
            Box::new(move |city: &CitiesRecord| -> Option<()> {
                if let Some(country) = &city.country {
                    if countries.contains(&country.code) {
                        Some(())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        } else {
            Box::new(|_city: &CitiesRecord| -> Option<()> { Some(()) })
        };

        if k != 0.0 {
            // use population as point weight
            let mut points = self
                .tree
                .nearest_n(&[loc.0, loc.1], limit, &squared_euclidean)
                .iter()
                .filter_map(|nearest| {
                    let city = self.geonames.get(&nearest.item)?;
                    filter_by_countries(city)?;
                    Some((
                        nearest.distance,
                        nearest.distance - k * city.population as f32,
                        city,
                    ))
                })
                .collect::<Vec<(f32, f32, &CitiesRecord)>>();

            // points.sort_by_key(|i| i.0);
            points.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            Some(
                points
                    .iter()
                    .take(limit)
                    .map(|p| ReverseItem {
                        distance: p.0,
                        score: p.1,
                        city: p.2,
                    })
                    .collect(),
            )
        } else {
            Some(
                self.tree
                    .nearest_n(&[loc.0, loc.1], limit, &squared_euclidean)
                    .iter()
                    .filter_map(|p| {
                        let city = self.geonames.get(&p.item)?;
                        filter_by_countries(city)?;
                        Some(ReverseItem {
                            distance: p.distance,
                            score: p.distance,
                            city,
                        })
                    })
                    .collect(),
            )
        }
    }

    pub fn new_from_files<P: AsRef<std::path::Path>>(
        SourceFileOptions {
            cities,
            names,
            countries,
            filter_languages,
            admin1_codes,
            admin2_codes,
        }: SourceFileOptions<P>,
        source_etag: HashMap<String, String>,
    ) -> Result<Self, Box<dyn Error>> {
        Engine::new_from_files_content(
            SourceFileContentOptions {
                cities: std::fs::read_to_string(cities)?,
                names: if let Some(p) = names {
                    Some(std::fs::read_to_string(p)?)
                } else {
                    None
                },
                countries: if let Some(p) = countries {
                    Some(std::fs::read_to_string(p)?)
                } else {
                    None
                },
                admin1_codes: if let Some(p) = admin1_codes {
                    Some(std::fs::read_to_string(p)?)
                } else {
                    None
                },
                admin2_codes: if let Some(p) = admin2_codes {
                    Some(std::fs::read_to_string(p)?)
                } else {
                    None
                },
                filter_languages,
            },
            source_etag,
        )
    }

    pub fn new_from_files_content(
        SourceFileContentOptions {
            cities,
            names,
            countries,
            filter_languages,
            admin1_codes,
            admin2_codes,
        }: SourceFileContentOptions,
        source_etag: HashMap<String, String>,
    ) -> Result<Self, Box<dyn Error>> {
        #[cfg(feature = "tracing")]
        let now = Instant::now();

        let mut entries: Vec<Entry> = Vec::new();
        let mut geonames: HashMap<u32, CitiesRecord> = HashMap::new();
        let mut capitals: HashMap<String, u32> = HashMap::new();

        let records = split_content_to_n_parts(&cities, rayon::current_num_threads())
            .par_iter()
            .map(|chunk| {
                let mut rdr = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .delimiter(b'\t')
                    .from_reader(chunk.as_bytes());

                rdr.deserialize()
                    .filter_map(|row| {
                        let record: CitiesRecordRaw = row.ok()?;
                        Some(record)
                    })
                    .collect::<Vec<CitiesRecordRaw>>()
            })
            .reduce(Vec::new, |mut m1, ref mut m2| {
                m1.append(m2);
                m1
            });

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Engine read {} cities took {}ms",
            records.len(),
            now.elapsed().as_millis(),
        );

        // load country info
        let country_by_code: Option<HashMap<String, Country>> = match countries {
            Some(contents) => {
                #[cfg(feature = "tracing")]
                let now = Instant::now();

                let contents = skip_comment_lines(contents);

                let mut rdr = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .delimiter(b'\t')
                    .from_reader(contents.as_bytes());

                let countries = rdr
                    .deserialize()
                    .filter_map(|row| {
                        let record: CountryInfoRaw = row
                            .map_err(|e| {
                                #[cfg(feature = "tracing")]
                                tracing::error!("On read country row: {e}");

                                e
                            })
                            .ok()?;
                        Some((
                            record.iso.clone(),
                            Country {
                                id: record.geonameid,
                                code: record.iso,
                                name: record.name,
                            },
                        ))
                    })
                    .collect::<HashMap<String, Country>>();

                #[cfg(feature = "tracing")]
                tracing::info!(
                    "Engine read {} countries took {}ms",
                    countries.len(),
                    now.elapsed().as_millis(),
                );

                Some(countries)
            }
            None => None,
        };

        // load admin1 code info
        let admin1_by_code: Option<HashMap<String, AdminDivision>> = match admin1_codes {
            Some(contents) => {
                #[cfg(feature = "tracing")]
                let now = Instant::now();

                let mut rdr = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .delimiter(b'\t')
                    .from_reader(contents.as_bytes());

                let admin_division = rdr
                    .deserialize()
                    .filter_map(|row| {
                        let record: Admin1CodeRecordRaw = row.ok()?;
                        Some((
                            record.code.clone(),
                            AdminDivision {
                                id: record.geonameid,
                                code: record.code,
                                name: record.name,
                            },
                        ))
                    })
                    .collect::<HashMap<String, AdminDivision>>();

                #[cfg(feature = "tracing")]
                tracing::info!(
                    "Engine read {} admin1 codes took {}ms",
                    admin_division.len(),
                    now.elapsed().as_millis(),
                );

                Some(admin_division)
            }
            None => None,
        };

        // load admin2 code info
        let admin2_by_code: Option<HashMap<String, AdminDivision>> = match admin2_codes {
            Some(contents) => {
                #[cfg(feature = "tracing")]
                let now = Instant::now();

                let mut rdr = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .delimiter(b'\t')
                    .from_reader(contents.as_bytes());

                let admin_division = rdr
                    .deserialize()
                    .filter_map(|row| {
                        let record: Admin2CodeRecordRaw = row.ok()?;
                        Some((
                            record.code.clone(),
                            AdminDivision {
                                id: record.geonameid,
                                code: record.code,
                                name: record.name,
                            },
                        ))
                    })
                    .collect::<HashMap<String, AdminDivision>>();

                #[cfg(feature = "tracing")]
                tracing::info!(
                    "Engine read {} admin2 codes took {}ms",
                    admin_division.len(),
                    now.elapsed().as_millis(),
                );

                Some(admin_division)
            }
            None => None,
        };

        let mut names_by_id: Option<HashMap<u32, HashMap<String, String>>> = match names {
            Some(contents) => {
                #[cfg(feature = "tracing")]
                let now = Instant::now();

                // collect ids for cities
                let city_geoids = records
                    .iter()
                    .map(|item| item.geonameid)
                    .collect::<HashSet<u32>>();

                let country_geoids = if let Some(ref country_by_code) = country_by_code {
                    country_by_code
                        .values()
                        .map(|item| item.id)
                        .collect::<HashSet<u32>>()
                } else {
                    HashSet::<u32>::new()
                };

                let admin1_geoids = if let Some(ref admin_codes) = admin1_by_code {
                    admin_codes
                        .values()
                        .map(|item| item.id)
                        .collect::<HashSet<u32>>()
                } else {
                    HashSet::<u32>::new()
                };

                let admin2_geoids = if let Some(ref admin_codes) = admin2_by_code {
                    admin_codes
                        .values()
                        .map(|item| item.id)
                        .collect::<HashSet<u32>>()
                } else {
                    HashSet::<u32>::new()
                };

                // TODO: split to N parts can split one geonameid and build not accurate index
                // use rayon::current_num_threads() instead of 1
                let names_by_id = split_content_to_n_parts(&contents, 1)
                    .par_iter()
                    .map(move |chunk| {
                        let mut rdr = csv::ReaderBuilder::new()
                            .has_headers(false)
                            .delimiter(b'\t')
                            .from_reader(chunk.as_bytes());

                        let mut names_by_id: HashMap<u32, HashMap<String, AlternateNamesRaw>> =
                            HashMap::new();

                        for row in rdr.deserialize() {
                            let record: AlternateNamesRaw =
                                if let Ok(r) = row { r } else { continue };

                            let is_city_name = city_geoids.contains(&record.geonameid);
                            let mut skip = !is_city_name;

                            if skip {
                                skip = !country_geoids.contains(&record.geonameid)
                            }

                            if skip {
                                skip = !admin1_geoids.contains(&record.geonameid)
                            }

                            if skip {
                                skip = !admin2_geoids.contains(&record.geonameid)
                            }

                            // entry not used
                            if skip {
                                continue;
                            }

                            // skip short not preferred names for cities
                            if is_city_name
                                && record.is_short_name == "1"
                                && record.is_prefered_name != "1"
                            {
                                continue;
                            }

                            if record.is_colloquial == "1" {
                                continue;
                            }
                            if record.is_historic == "1" {
                                continue;
                            }

                            // filter by languages
                            if !filter_languages.contains(&record.isolanguage.as_str()) {
                                continue;
                            }

                            let lang = record.isolanguage.to_owned();

                            if let Some(item) = names_by_id.get_mut(&record.geonameid) {
                                // don't overwrite preferred name
                                let is_current_prefered_name = item
                                    .get(&record.isolanguage)
                                    .map(|i| i.is_prefered_name == "1")
                                    .unwrap_or(false);

                                if !is_current_prefered_name {
                                    item.insert(lang, record);
                                }
                            } else {
                                let mut map: HashMap<String, AlternateNamesRaw> = HashMap::new();
                                let geonameid = record.geonameid;
                                map.insert(lang.to_owned(), record);
                                names_by_id.insert(geonameid, map);
                            }
                        }

                        // convert names to simple struct
                        let result: HashMap<u32, HashMap<String, String>> =
                            names_by_id.iter().fold(HashMap::new(), |mut acc, c| {
                                let (geonameid, names) = c;
                                acc.insert(
                                    *geonameid,
                                    names.iter().fold(
                                        HashMap::new(),
                                        |mut accn: HashMap<String, String>, n| {
                                            let (isolanguage, n) = n;
                                            accn.insert(
                                                isolanguage.to_owned(),
                                                n.alternate_name.to_owned(),
                                            );
                                            accn
                                        },
                                    ),
                                );
                                acc
                            });
                        result
                    })
                    .reduce(HashMap::new, |mut m1, m2| {
                        m1.extend(m2);
                        m1
                    });

                #[cfg(feature = "tracing")]
                tracing::info!(
                    "Engine read {} names took {}ms",
                    records.len(),
                    now.elapsed().as_millis(),
                );

                Some(names_by_id)
            }
            None => None,
        };

        let mut tree = KdTree::new();

        for record in records {
            // INCLUDE:
            // PPL	populated place	a city, town, village, or other agglomeration of buildings where people live and work
            // PPLA	seat of a first-order administrative division	seat of a first-order administrative division (PPLC takes precedence over PPLA)
            // PPLA2	seat of a second-order administrative division
            // PPLA3	seat of a third-order administrative division
            // PPLA4	seat of a fourth-order administrative division
            // PPLA5	seat of a fifth-order administrative division
            // PPLC	capital of a political entity
            // PPLS	populated places	cities, towns, villages, or other agglomerations of buildings where people live and work
            // PPLG	seat of government of a political entity
            // PPLCH	historical capital of a political entity	a former capital of a political entity
            //
            // EXCLUDE:
            // PPLF farm village	a populated place where the population is largely engaged in agricultural activities
            // PPLL	populated locality	an area similar to a locality but with a small group of dwellings or other buildings
            // PPLQ	abandoned populated place
            // PPLW	destroyed populated place	a village, town or city destroyed by a natural disaster, or by war
            // PPLX	section of populated place
            // STLMT israeli settlement

            let feature_code = record.feature_code.as_str();
            match feature_code {
                "PPLA3" | "PPLA4" | "PPLA5" | "PPLF" | "PPLL" | "PPLQ" | "PPLW" | "PPLX"
                | "STLMT" => continue,
                _ => {}
            };

            // prevent dublicates
            if geonames.contains_key(&record.geonameid) {
                continue;
            }

            tree.add(&[record.latitude, record.longitude], record.geonameid);

            let country_id = country_by_code
                .as_ref()
                .and_then(|m| m.get(&record.country_code).map(|c| c.id));

            entries.push(Entry {
                id: record.geonameid,
                value: record.name.to_lowercase().to_owned(),
                country_id,
            });

            if record.name != record.asciiname {
                entries.push(Entry {
                    id: record.geonameid,
                    value: record.asciiname.to_lowercase().to_owned(),
                    country_id,
                });
            }

            for altname in record.alternatenames.split(',') {
                entries.push(Entry {
                    id: record.geonameid,
                    value: altname.to_lowercase(),
                    country_id,
                });
            }

            let country = if let Some(ref c) = country_by_code {
                if feature_code == "PPLC" {
                    capitals.insert(
                        record.country_code.to_lowercase().to_string(),
                        record.geonameid,
                    );
                }
                c.get(&record.country_code).cloned()
            } else {
                None
            };

            let country_names = if let Some(ref c) = country {
                match names_by_id {
                    Some(ref names) => names.get(&c.id).cloned(),
                    None => None,
                }
            } else {
                None
            };

            let admin_division = if let Some(ref a) = admin1_by_code {
                a.get(&format!("{}.{}", record.country_code, record.admin1_code))
                    .cloned()
            } else {
                None
            };

            let admin1_names = if let Some(ref a) = admin_division {
                match names_by_id {
                    Some(ref names) => names.get(&a.id).cloned(),
                    None => None,
                }
            } else {
                None
            };

            let admin2_division = if let Some(ref a) = admin2_by_code {
                a.get(&format!(
                    "{}.{}.{}",
                    record.country_code, record.admin1_code, record.admin2_code
                ))
                .cloned()
            } else {
                None
            };

            let admin2_names = if let Some(ref a) = admin2_division {
                match names_by_id {
                    Some(ref names) => names.get(&a.id).cloned(),
                    None => None,
                }
            } else {
                None
            };

            geonames.insert(
                record.geonameid,
                CitiesRecord {
                    id: record.geonameid,
                    name: record.name,
                    country,
                    admin_division,
                    admin2_division,
                    latitude: record.latitude,
                    longitude: record.longitude,
                    timezone: record.timezone,
                    names: match names_by_id {
                        Some(ref mut names) => names.remove(&record.geonameid),
                        None => None,
                    },
                    country_names,
                    admin1_names,
                    admin2_names,
                    population: record.population,
                },
            );
        }

        let engine = Engine {
            source_etag,
            geonames,
            capitals,
            tree,
            entries,
            country_id_by_code: if let Some(country_by_code) = country_by_code {
                HashMap::from_iter(
                    country_by_code
                        .into_iter()
                        .map(|(code, country)| (code, country.id)),
                )
            } else {
                HashMap::new()
            },
            #[cfg(feature = "geoip2_support")]
            geoip2_reader: None,
        };

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Engine ready (entries {}, geonames {}, capitals {}). took {}ms",
            engine.entries.len(),
            engine.geonames.len(),
            engine.capitals.len(),
            now.elapsed().as_millis()
        );
        Ok(engine)
    }

    pub fn dump_to<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        format: EngineDumpFormat,
    ) -> std::io::Result<()> {
        #[cfg(feature = "tracing")]
        let now = Instant::now();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;

        match format {
            EngineDumpFormat::Json => serde_json::to_writer(file, self)?,
            EngineDumpFormat::Bincode => bincode::serialize_into(file, &self).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("bincode: {}", e))
            })?,
        };

        #[cfg(feature = "tracing")]
        {
            let metadata = std::fs::metadata(&path)?;

            tracing::info!(
                "Engine dump [{:?}] size: {} bytes. took {}ms",
                format,
                metadata.len(),
                now.elapsed().as_millis(),
            );
        }

        Ok(())
    }

    pub fn load_from<P: AsRef<std::path::Path>>(
        path: P,
        format: EngineDumpFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(feature = "tracing")]
        tracing::info!("Engine starts load index from file [{:?}]...", format);

        #[cfg(feature = "tracing")]
        let now = Instant::now();
        let file = std::fs::OpenOptions::new()
            .create(false)
            .read(true)
            .truncate(false)
            .open(&path)?;

        let engine_dump: EngineDump = match format {
            EngineDumpFormat::Json => serde_json::from_reader(file)?,
            EngineDumpFormat::Bincode => bincode::deserialize_from(file)?,
        };

        let mut tree = KdTree::new();
        for (geonameid, record) in &engine_dump.geonames {
            tree.add(&[record.latitude, record.longitude], *geonameid);
        }

        let engine = Engine {
            source_etag: engine_dump.source_etag,
            entries: engine_dump.entries,
            geonames: engine_dump.geonames,
            capitals: engine_dump.capitals,
            country_id_by_code: engine_dump.country_id_by_code,
            tree,
            #[cfg(feature = "geoip2_support")]
            geoip2_reader: None,
        };

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Engine loaded from file. took {}ms",
            now.elapsed().as_millis(),
        );

        Ok(engine)
    }

    // TODO slim mmdb size, we are needs only geonameid
    /// **unsafe** method to initialize geoip2 buffer and reader
    #[cfg(feature = "geoip2_support")]
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

    #[cfg(feature = "geoip2_support")]
    pub fn geoip2_lookup(&self, addr: IpAddr) -> Option<&CitiesRecord> {
        match self.geoip2_reader.as_ref() {
            Some((_, reader)) => {
                let result = reader.lookup(addr).ok()?;
                let city = result.city?;
                let id = city.geoname_id?;
                self.geonames.get(&id)
            }
            None => {
                #[cfg(feature = "tracing")]
                tracing::warn!("Geoip2 reader is't configured!");
                None
            }
        }
    }
}

fn split_content_to_n_parts(content: &str, n: usize) -> Vec<String> {
    if n == 0 || n == 1 {
        return vec![content.to_owned()];
    }

    let lines: Vec<&str> = content.lines().collect();
    lines.chunks(n).map(|chunk| chunk.join("\n")).collect()
}

#[cfg(feature = "geoip2_support")]
struct GeoIP2Error(geoip2::Error);

#[cfg(feature = "geoip2_support")]
impl std::error::Error for GeoIP2Error {}

#[cfg(feature = "geoip2_support")]
impl std::fmt::Debug for GeoIP2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[cfg(feature = "geoip2_support")]
impl std::fmt::Display for GeoIP2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GeoIP2 Error {:?}", self.0)
    }
}
