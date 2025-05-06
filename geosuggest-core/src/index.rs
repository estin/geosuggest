use itertools::Itertools;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::error::Error;

#[cfg(feature = "oaph")]
use oaph::schemars::{self, JsonSchema};

#[cfg(feature = "tracing")]
use std::time::Instant;

pub fn skip_comment_lines(content: &str) -> String {
    content.lines().filter(|l| !l.starts_with('#')).join("\n")
}

fn split_content_to_n_parts(content: &str, n: usize) -> Vec<String> {
    if n == 0 || n == 1 {
        return vec![content.to_owned()];
    }

    let lines: Vec<&str> = content.lines().collect();
    lines.chunks(n).map(|chunk| chunk.join("\n")).collect()
}

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

#[derive(Clone, rkyv::Deserialize, rkyv::Serialize, rkyv::Archive)]
pub struct IndexData {
    pub entries: Vec<Entry>,
    pub geonames: HashMap<u32, CitiesRecord>,
    pub capitals: HashMap<String, u32>,
    pub country_info_by_code: HashMap<String, CountryRecord>,
}

#[derive(Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
pub struct Entry {
    pub id: u32,                 // geoname id
    pub value: String,           // searchable value
    pub country_id: Option<u32>, // geoname country id
}

// code, name, name ascii, geonameid
#[derive(Debug, Clone, serde::Deserialize)]
struct Admin1CodeRecordRaw {
    code: String,
    name: String,
    _asciiname: String,
    geonameid: u32,
}

// code, name, name ascii, geonameid
#[derive(Debug, Clone, serde::Deserialize)]
struct Admin2CodeRecordRaw {
    code: String,
    name: String,
    _asciiname: String,
    geonameid: u32,
}

#[derive(Debug, Clone, serde::Serialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
#[cfg_attr(feature = "oaph", derive(JsonSchema))]
#[rkyv(derive(serde::Serialize, Debug))]
pub struct AdminDivision {
    #[rkyv(attr(serde(serialize_with = "serialize_archived_u32")))]
    pub id: u32,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub code: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
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

#[derive(Debug, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Deserialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
#[rkyv(derive(serde::Serialize, Debug))]
pub struct CountryRecordRaw {
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub iso: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub iso3: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub iso_numeric: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub fips: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub name: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub capital: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub area: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_u32")))]
    pub population: u32,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub continent: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub tld: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub currency_code: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub currency_name: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub phone: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub postal_code_format: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub postal_code_regex: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub languages: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_u32")))]
    pub geonameid: u32,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub neighbours: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub equivalent_fips_code: String,
}

#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
#[rkyv(derive(Debug, serde::Serialize))]
pub struct CountryRecord {
    /// geonames country info
    pub info: CountryRecordRaw,

    /// Country name translation
    #[rkyv(attr(serde(serialize_with = "serialize_archived_optional_map")))]
    pub names: Option<HashMap<String, String>>,

    /// Capital name translation
    #[rkyv(attr(serde(serialize_with = "serialize_archived_optional_map")))]
    pub capital_names: Option<HashMap<String, String>>,
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
#[derive(Debug, serde::Deserialize)]
struct AlternateNamesRaw {
    _alternate_name_id: u32,
    geonameid: u32,
    isolanguage: String,
    alternate_name: String,
    is_preferred_name: String,
    is_short_name: String,
    is_colloquial: String,
    is_historic: String,
    _from: String,
    _to: String,
}

#[cfg_attr(feature = "oaph", derive(JsonSchema))]
#[derive(Debug, Clone, serde::Serialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
#[rkyv(derive(serde::Serialize, Debug))]
pub struct Country {
    #[rkyv(attr(serde(serialize_with = "serialize_archived_u32")))]
    pub id: u32,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub code: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub name: String,
}

impl From<&CountryRecordRaw> for Country {
    fn from(c: &CountryRecordRaw) -> Self {
        Country {
            id: c.geonameid,
            code: c.iso.clone(),
            name: c.name.clone(),
        }
    }
}

#[cfg_attr(feature = "oaph", derive(JsonSchema))]
#[derive(Debug, Clone, serde::Serialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
#[rkyv(derive(serde::Serialize, Debug))]
pub struct CitiesRecord {
    #[rkyv(attr(serde(serialize_with = "serialize_archived_u32")))]
    pub id: u32,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub name: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_f32")))]
    pub latitude: f32,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_f32")))]
    pub longitude: f32,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_option")))]
    pub country: Option<Country>,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_option")))]
    pub admin_division: Option<AdminDivision>,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_option")))]
    pub admin2_division: Option<AdminDivision>,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_string")))]
    pub timezone: String,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_optional_map")))]
    pub names: Option<HashMap<String, String>>,
    // todo try reuse country info
    #[rkyv(attr(serde(serialize_with = "serialize_archived_optional_map")))]
    pub country_names: Option<HashMap<String, String>>,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_optional_map")))]
    pub admin1_names: Option<HashMap<String, String>>,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_optional_map")))]
    pub admin2_names: Option<HashMap<String, String>>,
    #[rkyv(attr(serde(serialize_with = "serialize_archived_u32")))]
    pub population: u32,
}

impl IndexData {
    pub fn new_from_files<P: AsRef<std::path::Path>>(
        SourceFileOptions {
            cities,
            names,
            countries,
            filter_languages,
            admin1_codes,
            admin2_codes,
        }: SourceFileOptions<P>,
    ) -> Result<Self, Box<dyn Error>> {
        Self::new_from_files_content(SourceFileContentOptions {
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
        })
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
    ) -> Result<Self, Box<dyn Error>> {
        #[cfg(feature = "tracing")]
        let now = Instant::now();

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

        let mut geonames: Vec<CitiesRecord> = Vec::with_capacity(records.len());
        let mut entries: Vec<Entry> = Vec::with_capacity(
            records.len()
                * if !filter_languages.is_empty() {
                    filter_languages.len()
                } else {
                    1
                },
        );

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Engine read {} cities took {}ms",
            records.len(),
            now.elapsed().as_millis(),
        );

        // load country info
        let country_by_code: Option<HashMap<String, CountryRecordRaw>> = match countries {
            Some(contents) => {
                #[cfg(feature = "tracing")]
                let now = Instant::now();

                let contents = skip_comment_lines(&contents);

                let mut rdr = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .delimiter(b'\t')
                    .from_reader(contents.as_bytes());

                let countries = rdr
                    .deserialize()
                    .filter_map(|row| {
                        let record: CountryRecordRaw = row
                            .map_err(|e| {
                                #[cfg(feature = "tracing")]
                                tracing::error!("On read country row: {e}");

                                e
                            })
                            .ok()?;
                        Some((record.iso.clone(), record))
                    })
                    .collect::<HashMap<String, CountryRecordRaw>>();

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
                        .map(|item| item.geonameid)
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
                            let record: AlternateNamesRaw = if let Ok(r) = row {
                                r
                            } else {
                                continue;
                            };

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
                                && record.is_preferred_name != "1"
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
                                let is_current_preferred_name = item
                                    .get(&record.isolanguage)
                                    .map(|i| i.is_preferred_name == "1")
                                    .unwrap_or(false);

                                if !is_current_preferred_name {
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

        let mut capitals: HashMap<String, u32> =
            HashMap::with_capacity(if let Some(items) = &country_by_code {
                items.len()
            } else {
                0
            });

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

            let is_capital = feature_code == "PPLC";

            let country_id = country_by_code
                .as_ref()
                .and_then(|m| m.get(&record.country_code).map(|c| c.geonameid));

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
                if is_capital {
                    capitals.insert(record.country_code.to_string(), record.geonameid);
                }
                c.get(&record.country_code).cloned()
            } else {
                None
            };

            let country_names = if let Some(ref c) = country {
                match names_by_id {
                    Some(ref names) => names.get(&c.geonameid).cloned(),
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
            geonames.push(CitiesRecord {
                id: record.geonameid,
                name: record.name,
                country: country.as_ref().map(Country::from),
                admin_division,
                admin2_division,
                latitude: record.latitude,
                longitude: record.longitude,
                timezone: record.timezone,
                names: match names_by_id {
                    Some(ref mut names) => {
                        if is_capital {
                            names.get(&record.geonameid).cloned()
                        } else {
                            // don't hold unused data
                            names.remove(&record.geonameid)
                        }
                    }
                    None => None,
                },
                country_names,
                admin1_names,
                admin2_names,
                population: record.population,
            });
        }

        geonames.sort_unstable_by_key(|item| item.id);
        geonames.dedup_by_key(|item| item.id);

        let data = IndexData {
            geonames: HashMap::from_iter(geonames.into_iter().map(|item| (item.id, item))),
            entries,
            country_info_by_code: if let Some(country_by_code) = country_by_code {
                HashMap::from_iter(country_by_code.into_iter().map(|(code, country)| {
                    let country_record = CountryRecord {
                        names: names_by_id
                            .as_ref()
                            .and_then(|names| names.get(&country.geonameid).cloned()),
                        capital_names: match names_by_id {
                            Some(ref names) => {
                                if let Some(city_id) = capitals.get(&country.iso) {
                                    names.get(city_id).cloned()
                                } else {
                                    None
                                }
                            }
                            None => None,
                        },
                        info: country,
                    };

                    (code, country_record)
                }))
            } else {
                HashMap::new()
            },
            capitals,
        };

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Index data ready (entries {}, geonames {}, capitals {}). took {}ms",
            data.entries.len(),
            data.geonames.len(),
            data.capitals.len(),
            now.elapsed().as_millis()
        );
        Ok(data)
    }
}

use serde::ser::{SerializeMap, Serializer};
fn serialize_archived_string<S>(
    value: &rkyv::string::ArchivedString,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(value.as_str())
}

fn serialize_archived_u32<S>(value: &rkyv::rend::u32_le, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u32(value.to_native())
}

fn serialize_archived_f32<S>(value: &rkyv::rend::f32_le, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_f32(value.to_native())
}

fn serialize_archived_option<S, T>(
    value: &rkyv::option::ArchivedOption<T>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: serde::Serialize,
{
    if let rkyv::option::ArchivedOption::Some(v) = value {
        s.serialize_some(v)
    } else {
        s.serialize_none()
    }
}

fn serialize_archived_optional_map<S>(
    value: &rkyv::option::ArchivedOption<
        rkyv::collections::swiss_table::ArchivedHashMap<
            rkyv::string::ArchivedString,
            rkyv::string::ArchivedString,
        >,
    >,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let rkyv::option::ArchivedOption::Some(v) = value {
        let mut map = s.serialize_map(v.len().into())?;
        for (key, value) in v.iter() {
            map.serialize_entry(key.as_str(), value.as_str())?;
        }
        map.end()
    } else {
        s.serialize_none()
    }
}
