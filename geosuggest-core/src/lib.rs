use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::Instant;

use kdtree::{distance::squared_euclidean, KdTree};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use strsim::jaro_winkler;

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
    feature_class: String,
    feature_code: String,
    country_code: String,
    cc2: String,
    admin1_code: String,
    admin2_code: String,
    admin3_code: String,
    admin4_code: String,
    population: String,
    elevation: String,
    dem: String,
    timezone: String,
    modification_date: String,
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
    alternate_name_id: usize,
    geonameid: usize,
    isolanguage: String,
    alternate_name: String,
    is_prefered_name: String,
    is_short_name: String,
    is_colloquial: String,
    is_historic: String,
    from: String,
    to: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "oaph_support", derive(JsonSchema))]
pub struct CitiesRecord {
    pub id: usize,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country_code: String,
    pub timezone: String,
    pub names: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct EngineDump {
    entries: Vec<(usize, String)>,
    geonames: HashMap<usize, CitiesRecord>,
}

#[derive(Serialize)]
pub struct Engine {
    entries: Vec<(usize, String)>,
    geonames: HashMap<usize, CitiesRecord>,

    #[serde(skip_serializing)]
    tree: KdTree<f64, usize, [f64; 2]>,
}

impl Engine {
    pub fn suggest(&self, pattern: &str, limit: usize) -> Vec<&CitiesRecord> {
        if limit == 0 {
            return Vec::new();
        }
        self.search(&pattern.to_lowercase(), limit)
            .iter()
            .filter_map(|item| self.geonames.get(item))
            .collect::<Vec<&CitiesRecord>>()
    }

    pub fn reverse(&self, loc: (f64, f64)) -> Option<&CitiesRecord> {
        let nearest = match self.tree.nearest(&[loc.0, loc.1], 1, &squared_euclidean) {
            Ok(nearest) => nearest,
            Err(error) => match error {
                kdtree::ErrorKind::WrongDimension => {
                    panic!("Internal error, kdtree::ErrorKind::WrongDimension should never occur")
                }
                kdtree::ErrorKind::NonFiniteCoordinate => return None,
                kdtree::ErrorKind::ZeroCapacity => {
                    panic!("Internal error, kdtree::ErrorKind::ZeroCapacity should never occur")
                }
            },
        };
        match nearest.get(0) {
            Some(nearest) => Some(self.geonames.get(nearest.1).unwrap()),
            None => None,
        }
    }

    fn search(&self, pattern: &str, limit: usize) -> Vec<usize> {
        // search on whole index
        let mut result = self
            .entries
            .par_iter()
            .filter_map(|item| {
                let score = jaro_winkler(&item.1, pattern);
                if score > 0.8 {
                    Some((item.0, score))
                } else {
                    None
                }
            })
            .collect::<Vec<(usize, f64)>>();

        // sort by score
        result.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap());

        // collect result
        let mut items: Vec<usize> = Vec::with_capacity(limit);
        let mut set: HashSet<usize> = HashSet::new();
        let mut count: usize = 0;
        for item in result {
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
                .map(|row| {
                    let record: CitiesRecordRaw = row.unwrap();
                    record
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

        let mut names_by_id: Option<HashMap<usize, HashMap<String, String>>> = match names {
            Some(names_path) => {
                let contents = std::fs::read_to_string(names_path)?;
                let now = Instant::now();

                // collect ids for cities
                let geoids = records
                    .iter()
                    .map(|item| item.geonameid)
                    .collect::<HashSet<usize>>();

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

                        for row in rdr.deserialize().into_iter() {
                            let record: AlternateNamesRaw = row.unwrap();

                            if !geoids.contains(&record.geonameid) {
                                continue;
                            }
                            // skip short names
                            if record.is_short_name == "1" {
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
            // prevent dublicates
            if geonames.contains_key(&record.geonameid) {
                continue;
            }
            tree.add([record.latitude, record.longitude], record.geonameid)
                .unwrap();

            entries.push((record.geonameid, record.name.to_lowercase().to_owned()));

            if record.name != record.asciiname {
                entries.push((
                    record.geonameid,
                    record.asciiname.to_ascii_lowercase().to_owned(),
                ));
            }

            for altname in record.alternatenames.split(',').into_iter() {
                entries.push((record.geonameid, altname.to_lowercase().to_owned()));
            }
            geonames.insert(
                record.geonameid,
                CitiesRecord {
                    id: record.geonameid,
                    name: record.name,
                    country_code: record.country_code,
                    latitude: record.latitude,
                    longitude: record.longitude,
                    timezone: record.timezone,
                    names: match names_by_id {
                        Some(ref mut names) => names.remove(&record.geonameid),
                        None => None,
                    },
                },
            );
        }

        let engine = Engine {
            geonames,
            tree,
            entries,
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

    pub fn load_from_json<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<Self> {
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
            tree.add([record.latitude, record.longitude], *geonameid)
                .unwrap();
        }

        let engine = Engine {
            entries: engine_dump.entries,
            geonames: engine_dump.geonames,
            tree,
        };

        log::info!(
            "Engine loaded from file. took {}ms",
            now.elapsed().as_millis(),
        );

        Ok(engine)
    }
}

fn split_content_to_n_parts(content: &str, n: usize) -> Vec<&str> {
    if n == 0 || n == 1 {
        return vec![content];
    }

    let mut parts = Vec::new();
    let chunk_size = content.len() / n;

    let mut position: usize = 0;
    for _i in 0..n {
        if position >= content.len() {
            break;
        }
        let start_position = position;
        let chunk = {
            let mut offset = 0;
            loop {
                if let Some(c) = content.get(start_position..(start_position + chunk_size - offset))
                {
                    break c;
                }
                offset += 1;
            }
        };
        position = match chunk.rfind('\n') {
            Some(p) => start_position + p,
            None => content.len(),
        };
        parts.push(&content[start_position..position]);
    }

    parts
}
