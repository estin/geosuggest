use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::Instant;

use kdtree::{distance::squared_euclidean, KdTree};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strsim::jaro_winkler;

#[cfg(feature = "geoip2_support")]
use std::net::IpAddr;

#[cfg(feature = "geoip2_support")]
use geoip2::{City, Reader};

#[cfg(feature = "oaph_support")]
use oaph::schemars::{self, JsonSchema};

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
    geonameid: usize,
    name: String,
    asciiname: String,
    alternatenames: String,
    latitude: f64,
    longitude: f64,
    _feature_class: String,
    feature_code: String,
    country_code: String,
    _cc2: String,
    _admin1_code: String,
    _admin2_code: String,
    _admin3_code: String,
    _admin4_code: String,
    population: usize,
    _elevation: String,
    _dem: String,
    timezone: String,
    _modification_date: String,
}

// CounntryInfo
// http://download.geonames.org/export/dump/countryInfo.txt
// iso alpha2      iso alpha3      iso numeric     fips code       name    capital areaInSqKm      population      continent       languages       currency        geonameId
// RU      RUS     643     RS      Russia  Moscow  1.71E7  144478050       EU      ru,tt,xal,cau,ady,kv,ce,tyv,cv,udm,tut,mns,bua,myv,mdf,chm,ba,inh,tut,kbd,krc,av,sah,nog        RUB     2017370
#[derive(Debug, Deserialize)]
struct CountryInfoRaw {
    iso: String,
    _iso3: String,
    _iso_numeric: String,
    _fips: String,
    name: String,
    _capital: String,
    _area: String,
    _population: usize,
    _continent: String,
    _languages: String,
    _currency: String,
    geonameid: usize,
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
    _alternate_name_id: usize,
    geonameid: usize,
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
    pub id: usize,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "oaph_support", derive(JsonSchema))]
pub struct CitiesRecord {
    pub id: usize,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: Option<Country>,
    pub timezone: String,
    pub names: Option<HashMap<String, String>>,
    pub country_names: Option<HashMap<String, String>>,
    pub population: usize,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "oaph_support", derive(JsonSchema))]
pub struct ReverseItem<'a> {
    pub city: &'a CitiesRecord,
    pub distance: f64,
    pub score: f64,
}

#[derive(Deserialize)]
struct EngineDump {
    entries: Vec<(usize, String)>,
    geonames: HashMap<usize, CitiesRecord>,
}

#[derive(Debug, Deserialize)]
pub struct KdTreeEntry {
    pub geonameid: usize,
    pub population: usize,
}

#[derive(Serialize)]
pub struct Engine {
    entries: Vec<(usize, String)>,
    geonames: HashMap<usize, CitiesRecord>,

    #[serde(skip_serializing)]
    tree: KdTree<f64, KdTreeEntry, [f64; 2]>,

    #[cfg(feature = "geoip2_support")]
    #[serde(skip_serializing)]
    geoip2_reader: Option<(&'static Vec<u8>, &'static Reader<'static, City<'static>>)>,
}

impl Engine {
    pub fn get(&self, id: &usize) -> Option<&CitiesRecord> {
        self.geonames.get(id)
    }

    pub fn suggest(
        &self,
        pattern: &str,
        limit: usize,
        min_score: Option<f64>,
    ) -> Vec<&CitiesRecord> {
        if limit == 0 {
            return Vec::new();
        }
        self.search(&pattern.to_lowercase(), limit, min_score)
            .iter()
            .filter_map(|item| self.geonames.get(item))
            .collect::<Vec<&CitiesRecord>>()
    }

    fn _nearest(&self, loc: (f64, f64), limit: usize) -> Option<Vec<(f64, &KdTreeEntry)>> {
        match self
            .tree
            .nearest(&[loc.0, loc.1], limit, &squared_euclidean)
        {
            Ok(nearest) => Some(nearest),
            Err(error) => match error {
                kdtree::ErrorKind::WrongDimension => {
                    log::error!(
                        "Internal error, kdtree::ErrorKind::WrongDimension should never occur"
                    );
                    None
                }
                kdtree::ErrorKind::NonFiniteCoordinate => None,
                kdtree::ErrorKind::ZeroCapacity => {
                    log::error!(
                        "Internal error, kdtree::ErrorKind::ZeroCapacity should never occur"
                    );
                    None
                }
            },
        }
    }

    pub fn reverse(
        &self,
        loc: (f64, f64),
        limit: usize,
        k: Option<f64>,
    ) -> Option<Vec<ReverseItem>> {
        if limit == 0 {
            return None;
        }
        let k = k.unwrap_or(0.0);
        if k != 0.0 {
            // use population as point weight
            let mut points = self
                // find N * 10 cities and sort them by score
                ._nearest(loc, limit * 10)?
                .iter()
                .map(|item| {
                    (
                        item.0,
                        item.0 - k * item.1.population as f64,
                        item.1.geonameid,
                    )
                })
                .collect::<Vec<(f64, f64, usize)>>();

            // points.sort_by_key(|i| i.0);
            points.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            Some(
                points
                    .iter()
                    .take(limit)
                    .filter_map(|p| {
                        Some(ReverseItem {
                            distance: p.0,
                            score: p.1,
                            city: self.geonames.get(&p.2)?,
                        })
                    })
                    .collect(),
            )
        } else {
            Some(
                self._nearest(loc, limit)?
                    .iter()
                    .filter_map(|p| {
                        Some(ReverseItem {
                            distance: p.0,
                            score: p.0,
                            city: self.geonames.get(&p.1.geonameid)?,
                        })
                    })
                    .collect(),
            )
        }
    }

    fn search(&self, pattern: &str, limit: usize, min_score: Option<f64>) -> Vec<usize> {
        let min_score = min_score.unwrap_or(0.8);
        // search on whole index
        let mut result = self
            .entries
            .par_iter()
            .filter_map(|item| {
                let score = jaro_winkler(&item.1, pattern);
                if score > min_score {
                    Some((item.0, score))
                } else {
                    None
                }
            })
            .collect::<Vec<(usize, f64)>>();

        // sort by score
        result.sort_by(|lhs, rhs| {
            rhs.1
                .partial_cmp(&lhs.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // collect result
        let mut items: Vec<usize> = Vec::with_capacity(limit);
        let mut set: HashSet<usize> = HashSet::new();
        let mut count: usize = 0;
        for item in result {
            // exclude dublicates
            if set.contains(&item.0) {
                continue;
            }
            set.insert(item.0);
            items.push(item.0);
            count += 1;
            if count >= limit {
                break;
            }
        }
        items
    }

    pub fn new_from_files<P: AsRef<std::path::Path>>(
        cities: P,
        names: Option<P>,
        countries: Option<P>,
        filter_languages: Vec<&str>,
    ) -> Result<Self, Box<dyn Error>> {
        let now = Instant::now();

        let mut entries: Vec<(usize, String)> = Vec::new();
        let mut geonames: HashMap<usize, CitiesRecord> = HashMap::new();

        let records = split_content_to_n_parts(
            &std::fs::read_to_string(cities)?,
            rayon::current_num_threads(),
        )
        .par_iter()
        .map(|chunk| {
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(false)
                .delimiter(b'\t')
                .from_reader(chunk.as_bytes());

            rdr.deserialize()
                .into_iter()
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

        log::info!(
            "Engine read {} cities took {}ms",
            records.len(),
            now.elapsed().as_millis(),
        );

        // load country info
        let country_by_code: Option<HashMap<String, Country>> = match countries {
            Some(counties_path) => {
                let contents = std::fs::read_to_string(counties_path)?;
                let now = Instant::now();

                let mut rdr = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .delimiter(b'\t')
                    .from_reader(contents.as_bytes());

                let countries = rdr
                    .deserialize()
                    .into_iter()
                    .filter_map(|row| {
                        let record: CountryInfoRaw = row.ok()?;
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

                log::info!(
                    "Engine read {} countries took {}ms",
                    countries.len(),
                    now.elapsed().as_millis(),
                );

                Some(countries)
            }
            None => None,
        };

        let mut names_by_id: Option<HashMap<usize, HashMap<String, String>>> = match names {
            Some(names_path) => {
                let contents = std::fs::read_to_string(names_path)?;
                let now = Instant::now();

                // collect ids for cities
                let city_geoids = records
                    .iter()
                    .map(|item| item.geonameid)
                    .collect::<HashSet<usize>>();

                let country_geoids = if let Some(ref country_by_code) = country_by_code {
                    country_by_code
                        .values()
                        .map(|item| item.id)
                        .collect::<HashSet<usize>>()
                } else {
                    HashSet::<usize>::new()
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

                        let mut names_by_id: HashMap<usize, HashMap<String, AlternateNamesRaw>> =
                            HashMap::new();

                        // cities
                        for row in rdr.deserialize() {
                            let record: AlternateNamesRaw =
                                if let Ok(r) = row { r } else { continue };

                            let is_city_name = city_geoids.contains(&record.geonameid);
                            let is_country_name = country_geoids.contains(&record.geonameid);

                            if !is_city_name && !is_country_name {
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
                        let result: HashMap<usize, HashMap<String, String>> =
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

                log::info!(
                    "Engine read {} names took {}ms",
                    records.len(),
                    now.elapsed().as_millis(),
                );

                Some(names_by_id)
            }
            None => None,
        };

        let mut tree = KdTree::with_capacity(2, records.len());

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
            //
            match record.feature_code.as_str() {
                "PPLA3" | "PPLA4" | "PPLA5" | "PPLF" | "PPLL" | "PPLQ" | "PPLW" | "PPLX"
                | "STLMT" => continue,
                _ => {}
            };

            // prevent dublicates
            if geonames.contains_key(&record.geonameid) {
                continue;
            }

            tree.add(
                [record.latitude, record.longitude],
                KdTreeEntry {
                    geonameid: record.geonameid,
                    population: record.population,
                },
            )?;

            entries.push((record.geonameid, record.name.to_lowercase().to_owned()));

            if record.name != record.asciiname {
                entries.push((
                    record.geonameid,
                    record.asciiname.to_ascii_lowercase().to_owned(),
                ));
            }

            for altname in record.alternatenames.split(',') {
                entries.push((record.geonameid, altname.to_lowercase().to_owned()));
            }

            let country = if let Some(ref c) = country_by_code {
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

            geonames.insert(
                record.geonameid,
                CitiesRecord {
                    id: record.geonameid,
                    name: record.name,
                    country,
                    latitude: record.latitude,
                    longitude: record.longitude,
                    timezone: record.timezone,
                    names: match names_by_id {
                        Some(ref mut names) => names.remove(&record.geonameid),
                        None => None,
                    },
                    country_names,
                    population: record.population,
                },
            );
        }

        let engine = Engine {
            geonames,
            tree,
            entries,
            #[cfg(feature = "geoip2_support")]
            geoip2_reader: None,
        };

        log::info!(
            "Engine ready (entries {}, geonames {}). took {}ms",
            engine.entries.len(),
            engine.geonames.len(),
            now.elapsed().as_millis()
        );
        Ok(engine)
    }

    pub fn dump_to_json<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        let now = Instant::now();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        serde_json::to_writer(file, self)?;

        let metadata = std::fs::metadata(&path)?;
        log::info!(
            "Engine dump size: {} bytes. took {}ms",
            metadata.len(),
            now.elapsed().as_millis(),
        );

        Ok(())
    }

    pub fn load_from_json<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("Engine starts load index from file...");

        let now = Instant::now();
        let file = std::fs::OpenOptions::new()
            .create(false)
            .read(true)
            .truncate(false)
            .open(&path)?;

        let engine_dump: EngineDump = serde_json::from_reader(file)?;

        let mut tree = KdTree::with_capacity(2, engine_dump.geonames.len());
        for (geonameid, record) in &engine_dump.geonames {
            tree.add(
                [record.latitude, record.longitude],
                KdTreeEntry {
                    population: record.population,
                    geonameid: *geonameid,
                },
            )?
        }

        let engine = Engine {
            entries: engine_dump.entries,
            geonames: engine_dump.geonames,
            tree,
            #[cfg(feature = "geoip2_support")]
            geoip2_reader: None,
        };

        log::info!(
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
                let id = city.geoname_id? as usize;
                self.geonames.get(&id)
            }
            None => {
                log::warn!("Geoip2 reader is't configured!");
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
