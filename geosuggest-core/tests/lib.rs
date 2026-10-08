use geosuggest_core::{
    index::{ArchivedCitiesRecord, IndexData, SourceFileOptions},
    storage, EngineData, EngineMetadata,
};
use std::{env::temp_dir, error::Error};

#[cfg(feature = "geoip2")]
use std::{net::IpAddr, str::FromStr};

fn get_engine_data(
    cities: Option<&str>,
    names: Option<&str>,
    countries: Option<&str>,
    filter_languages: Vec<&str>,
) -> Result<geosuggest_core::EngineData, Box<dyn Error>> {
    get_engine_data_with_excluded(
        cities,
        names,
        countries,
        filter_languages,
        geosuggest_core::index::DEFAULT_EXCLUDED_FEATURE_CODES.to_vec(),
    )
}

fn get_engine_data_with_excluded(
    cities: Option<&str>,
    names: Option<&str>,
    countries: Option<&str>,
    filter_languages: Vec<&str>,
    excluded_feature_codes: Vec<&str>,
) -> Result<geosuggest_core::EngineData, Box<dyn Error>> {
    let data = IndexData::new_from_files(SourceFileOptions {
        cities: cities.unwrap_or("tests/misc/cities.txt"),
        names: Some(names.unwrap_or("tests/misc/names.txt")),
        countries: Some(countries.unwrap_or("tests/misc/country-info.txt")),
        filter_languages,
        admin1_codes: Some("tests/misc/admin1-codes.txt"),
        admin2_codes: Some("tests/misc/admin2-codes.txt"),
        excluded_feature_codes,
    })?;

    let mut engine_data = EngineData::try_from(data)?;

    engine_data.metadata = Some(EngineMetadata::default());
    Ok(engine_data)
}

#[test_log::test]
fn suggest() -> Result<(), Box<dyn Error>> {
    let engine_data = get_engine_data(None, None, None, vec![])?;
    let engine = engine_data.as_engine()?;

    let items = engine.suggest::<&str>("voronezh", 1, None, None);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Voronezh");
    assert_eq!(engine.city_country(items[0]).unwrap().name, "Russia");
    assert_eq!(engine.city_admin1(items[0]).unwrap().name, "Voronezj");

    let items = engine.suggest::<&str>("Beverley", 1, None, None);
    tracing::info!("Items {items:#?}");
    assert_eq!(items[0].name, "Beverley");
    assert_eq!(
        engine.city_admin2(items[0]).unwrap().name,
        "East Riding of Yorkshire"
    );

    let items = engine.suggest("Beverley", 1, None, Some(&["RU"]));
    assert_eq!(items.len(), 0);

    let items = engine.suggest("Beverley", 1, None, Some(&["GB"]));
    assert_eq!(items.len(), 1);

    Ok(())
}

#[test_log::test]
fn suggest_topk_invariants() -> Result<(), Box<dyn Error>> {
    let engine_data = get_engine_data(None, None, None, vec![])?;
    let engine = engine_data.as_engine()?;

    fn ids(items: &[&ArchivedCitiesRecord]) -> Vec<u32> {
        items.iter().map(|c| c.id.to_native()).collect()
    }

    for pattern in ["voronezh", "Beverley", "o", "e", "mos", "zzz-no-match", ""] {
        for min_score in [None, Some(0.5), Some(0.99)] {
            let full = ids(&engine.suggest::<&str>(pattern, 50, min_score, None));
            for limit in [1usize, 2, 3, 5] {
                let part = ids(&engine.suggest::<&str>(pattern, limit, min_score, None));
                // bounded: never more than asked
                assert!(part.len() <= limit, "{pattern} {limit}");
                // exact: top-K is a prefix of top-50
                assert_eq!(&full[..part.len()], &part[..], "{pattern} {limit}");
                // distinct cities
                let mut seen = std::collections::HashSet::new();
                assert!(part.iter().all(|id| seen.insert(id)), "{pattern} {limit}");
                // deterministic across runs
                let again = ids(&engine.suggest::<&str>(pattern, limit, min_score, None));
                assert_eq!(part, again, "{pattern} {limit}");
            }
        }
        // country filter keeps only matching countries
        for code in ["RU", "GB"] {
            let items = engine.suggest(pattern, 10, None, Some(&[code]));
            for city in &items {
                assert_eq!(
                    engine.city_country(city).unwrap().code.as_str(),
                    code,
                    "{pattern} {code}"
                );
            }
        }
    }

    Ok(())
}

#[test_log::test]
fn suggest_population_tiebreak() -> Result<(), Box<dyn Error>> {
    use geosuggest_core::index::SourceFileContentOptions;

    // two exact-prefix matches: same score 1.0, different populations
    let cities = "1\tAbc\tAbc\t\t55.0\t37.0\tP\tPPL\tRU\t\t\t\t\t\t100\t\t\tEurope/Moscow\t2020-01-01\n\
                  2\tAbcd\tAbcd\t\t55.1\t37.1\tP\tPPL\tRU\t\t\t\t\t\t1000000\t\t\tEurope/Moscow\t2020-01-01\n";
    let data = IndexData::new_from_files_content(SourceFileContentOptions {
        cities: cities.to_owned(),
        names: None,
        countries: None,
        admin1_codes: None,
        admin2_codes: None,
        filter_languages: vec![],
        excluded_feature_codes:
            geosuggest_core::index::DEFAULT_EXCLUDED_FEATURE_CODES.to_vec(),
    })?;
    let engine_data = EngineData::try_from(data)?;
    let engine = engine_data.as_engine()?;

    let items = engine.suggest::<&str>("abc", 2, None, None);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Abcd");
    assert_eq!(items[1].name, "Abc");

    Ok(())
}

#[test_log::test]
fn reverse_country_rounds() -> Result<(), Box<dyn Error>> {
    let engine_data = get_engine_data(None, None, None, vec![])?;
    let engine = engine_data.as_engine()?;
    let total = engine.data.geonames.len();

    // a filtered query must return the same cities as filtering the full
    // unfiltered list, no matter how many fetch rounds it takes; Voronezh
    // coordinates with a GB filter force several rounds
    for loc in [(51.6372, 39.1937), (53.84587, -0.42332)] {
        let all = engine.reverse::<&str>(loc, total, None, None).unwrap();
        for limit in [1usize, 2, 10] {
            for codes in [&["GB"][..], &["RU"][..], &["GB", "RU"][..], &["XX"][..]] {
                let filtered = engine
                    .reverse(loc, limit, None, Some(&codes))
                    .unwrap_or_default();
                let expected: Vec<u32> = all
                    .iter()
                    .filter(|r| {
                        engine
                            .city_country(r.city)
                            .map(|c| codes.contains(&c.code.as_str()))
                            .unwrap_or(false)
                    })
                    .take(limit)
                    .map(|r| r.city.id.to_native())
                    .collect();
                let got: Vec<u32> =
                    filtered.iter().map(|r| r.city.id.to_native()).collect();
                assert_eq!(got, expected, "{loc:?} {limit} {codes:?}");
            }
        }
    }

    Ok(())
}

#[test_log::test]
fn excluded_feature_codes_changes_index() -> Result<(), Box<dyn Error>> {
    // Beverley is PPLA2 (included by default list).
    let default_data = get_engine_data(None, None, None, vec![])?;
    let default_engine = default_data.as_engine()?;
    assert_eq!(default_engine.suggest::<&str>("Beverley", 1, None, None).len(), 1);

    // Excluding PPLA2 drops it from the index.
    let custom_data = get_engine_data_with_excluded(None, None, None, vec![], vec!["PPLA2"])?;
    let custom_engine = custom_data.as_engine()?;
    assert_eq!(custom_engine.suggest::<&str>("Beverley", 1, None, None).len(), 0);

    // Empty list excludes nothing: Beverley is back.
    let all_data = get_engine_data_with_excluded(None, None, None, vec![], vec![])?;
    let all_engine = all_data.as_engine()?;
    assert_eq!(all_engine.suggest::<&str>("Beverley", 1, None, None).len(), 1);

    Ok(())
}

#[test_log::test]
fn reverse() -> Result<(), Box<dyn Error>> {
    let engine_data = get_engine_data(None, None, None, vec![])?;
    let engine = engine_data.as_engine()?;
    let result = engine.reverse::<&str>((51.6372, 39.1937), 1, None, None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].city.name, "Voronezh");
    assert_eq!(engine.city_country(items[0].city).unwrap().name, "Russia");
    assert_eq!(
        engine.city_admin1(items[0].city).unwrap().name,
        "Voronezj"
    );

    let result = engine.reverse::<&str>((53.84587, -0.42332), 1, None, None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].city.name, "Beverley");
    assert_eq!(
        engine.city_admin2(items[0].city).unwrap().name,
        "East Riding of Yorkshire"
    );

    let result = engine.reverse((53.84587, -0.42332), 1, None, Some(&["AR"]));
    assert_eq!(result.unwrap().len(), 0);

    let result = engine.reverse((53.84587, -0.42332), 1, None, Some(&["GB"]));
    assert_eq!(result.unwrap().len(), 1);

    Ok(())
}

#[test_log::test]
fn capital() -> Result<(), Box<dyn Error>> {
    let engine_data = get_engine_data(None, None, None, vec![])?;
    let engine = engine_data.as_engine()?;
    let result = engine.capital("RU");
    assert!(result.is_some());
    let city = result.unwrap();
    assert_eq!(city.name, "Moscow");
    assert_eq!(engine.city_country(city).unwrap().name, "Russia");
    Ok(())
}

#[test_log::test]
#[cfg(feature = "geoip2")]
fn geoip2_lookup() -> Result<(), Box<dyn Error>> {
    let mut engine_data = get_engine_data(None, None, None, vec![])?;
    engine_data.load_geoip2("tests/misc/GeoLite2-City-Test.mmdb")?;

    let engine = engine_data.as_engine()?;

    let result = engine.geoip2_lookup(IpAddr::from_str("81.2.69.142")?);
    assert!(result.is_some());
    let item = result.unwrap();
    assert_eq!(item.name, "London");

    Ok(())
}

#[test_log::test]
fn build_dump_load() -> Result<(), Box<dyn Error>> {
    let filepath = temp_dir().join("test-engine.rkyv");
    let storage = storage::Storage::new();
    // build
    let engine_data = get_engine_data(None, None, None, vec![])?;
    let engine = engine_data.as_engine()?;

    // dump
    storage.dump_to(&filepath, &engine_data)?;

    // check metadata
    let metadata = storage.read_metadata(&filepath)?;
    assert!(metadata.is_some());

    // load
    let from_dump = storage.load_from(&filepath)?;

    let from_dump_engine = from_dump.as_engine()?;

    assert_eq!(
        engine.suggest::<&str>("voronezh", 100, None, None).len(),
        from_dump_engine
            .suggest::<&str>("voronezh", 100, None, None)
            .len(),
    );

    let coords = (51.6372, 39.1937);
    assert_eq!(
        engine.reverse::<&str>(coords, 1, None, None).unwrap()[0]
            .city
            .id,
        from_dump_engine
            .reverse::<&str>(coords, 1, None, None)
            .unwrap()[0]
            .city
            .id,
    );

    Ok(())
}

#[test_log::test]
fn load_legacy_format() -> Result<(), Box<dyn Error>> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let engine_data = get_engine_data(None, None, None, vec![])?;

    // legacy layout written without `tracing`: length prefix + payload,
    // with the metadata bytes missing
    let filepath = temp_dir().join("test-engine-legacy.rkyv");
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filepath)?;
        let metadata = rkyv::to_bytes::<rkyv::rancor::Error>(&engine_data.metadata)?;
        file.write_all(&(metadata.len() as u32).to_be_bytes())?;
        file.write_all(&engine_data.data)?;
    }

    let loaded = storage::Storage::new().load_from(&filepath)?;
    assert_eq!(loaded.data.len(), engine_data.data.len());

    let engine = loaded.as_engine()?;
    let items = engine.suggest::<&str>("voronezh", 1, None, None);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Voronezh");

    Ok(())
}

#[test_log::test]
fn population_weight() -> Result<(), Box<dyn Error>> {
    let engine_data =
        get_engine_data(Some("tests/misc/population-weight.txt"), None, None, vec![])?;

    let engine = engine_data.as_engine()?;

    let population_weight = 0.000000005;

    // {
    //  "id": 532535,
    //  "name": "Lyublino",
    //  "country_code": "RU",
    //  "timezone": "Europe/Moscow",
    //  "latitude": 55.67738,
    //  "longitude": 37.76005
    // }

    // without weight coefficient
    let result = engine.reverse::<&str>((55.67738, 37.76006), 5, None, None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 3);
    tracing::trace!("Reverse result: {:#?}", items);
    assert_eq!(items[0].city.name, "Lyublino");

    // with weight coefficient
    let result = engine.reverse::<&str>((55.67738, 37.76006), 5, Some(population_weight), None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 3);
    tracing::trace!("Reverse result: {:#?}", items);
    assert_eq!(items[0].city.name, "Moscow");

    // {
    //   "id": 532615,
    //   "name": "Lyubertsy",
    //   "country_code": "RU",
    //   "timezone": "Europe/Moscow",
    //   "latitude": 55.67719,
    //   "longitude": 37.89322
    // }

    // with weight coefficient
    let result = engine.reverse::<&str>((55.67719, 37.89322), 5, Some(population_weight), None);
    assert!(result.is_some());
    let items = result.unwrap();
    tracing::trace!("Reverse result: {:#?}", items);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].city.name, "Lyubertsy");

    Ok(())
}

#[test_log::test]
fn country_info() -> Result<(), Box<dyn Error>> {
    let engine_data = get_engine_data(None, None, None, vec!["ru", "sr"])?;
    let engine = engine_data.as_engine()?;

    let country1 = engine.country_info("RS").unwrap();

    assert_eq!(country1.info.name, "Serbia");
    assert_eq!(
        country1.names.as_ref().unwrap().get("ru").unwrap(),
        "Сербия"
    );
    assert_eq!(
        country1.capital_names.as_ref().unwrap().get("ru").unwrap(),
        "Белград"
    );

    Ok(())
}
