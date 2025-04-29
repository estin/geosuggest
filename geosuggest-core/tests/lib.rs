use geosuggest_core::{storage, Engine, EngineMetadata, SourceFileOptions};
use std::{env::temp_dir, error::Error};

#[cfg(feature = "geoip2")]
use std::{net::IpAddr, str::FromStr};

fn get_engine(
    cities: Option<&str>,
    names: Option<&str>,
    countries: Option<&str>,
    filter_languages: Vec<&str>,
) -> Result<geosuggest_core::Engine, Box<dyn Error>> {
    let mut engine = Engine::new_from_files(SourceFileOptions {
        cities: cities.unwrap_or("tests/misc/cities.txt"),
        names: Some(names.unwrap_or("tests/misc/names.txt")),
        countries: Some(countries.unwrap_or("tests/misc/country-info.txt")),
        filter_languages,
        admin1_codes: Some("tests/misc/admin1-codes.txt"),
        admin2_codes: Some("tests/misc/admin2-codes.txt"),
    })?;
    engine.metadata = Some(EngineMetadata::default());
    Ok(engine)
}

#[test_log::test]
fn suggest() -> Result<(), Box<dyn Error>> {
    let engine = get_engine(None, None, None, vec![])?;

    let items = engine.suggest::<&str>("voronezh", 1, None, None);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Voronezh");
    assert_eq!(items[0].country.as_ref().unwrap().name, "Russia");
    assert_eq!(items[0].admin_division.as_ref().unwrap().name, "Voronezj");

    let items = engine.suggest::<&str>("Beverley", 1, None, None);
    tracing::info!("Items {items:#?}");
    assert_eq!(items[0].name, "Beverley");
    assert_eq!(
        items[0].admin2_division.as_ref().unwrap().name,
        "East Riding of Yorkshire"
    );

    let items = engine.suggest("Beverley", 1, None, Some(&["ru"]));
    assert_eq!(items.len(), 0);

    let items = engine.suggest("Beverley", 1, None, Some(&["gb"]));
    assert_eq!(items.len(), 1);

    Ok(())
}

#[test_log::test]
fn reverse() -> Result<(), Box<dyn Error>> {
    let engine = get_engine(None, None, None, vec![])?;
    let result = engine.reverse::<&str>((51.6372, 39.1937), 1, None, None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].city.name, "Voronezh");
    assert_eq!(items[0].city.country.as_ref().unwrap().name, "Russia");
    assert_eq!(
        items[0].city.admin_division.as_ref().unwrap().name,
        "Voronezj"
    );

    let result = engine.reverse::<&str>((53.84587, -0.42332), 1, None, None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].city.name, "Beverley");
    assert_eq!(
        items[0].city.admin2_division.as_ref().unwrap().name,
        "East Riding of Yorkshire"
    );

    let result = engine.reverse((53.84587, -0.42332), 1, None, Some(&["ar"]));
    assert_eq!(result.unwrap().len(), 0);

    let result = engine.reverse((53.84587, -0.42332), 1, None, Some(&["gb"]));
    assert_eq!(result.unwrap().len(), 1);

    Ok(())
}

#[test_log::test]
fn capital() -> Result<(), Box<dyn Error>> {
    let engine = get_engine(None, None, None, vec![])?;
    let result = engine.capital("RU");
    assert!(result.is_some());
    let city = result.unwrap();
    assert_eq!(city.name, "Moscow");
    assert_eq!(city.country.as_ref().unwrap().name, "Russia");
    Ok(())
}

#[test_log::test]
#[cfg(feature = "geoip2")]
fn geoip2_lookup() -> Result<(), Box<dyn Error>> {
    let mut engine = get_engine(None, None, None, vec![])?;
    engine.load_geoip2("tests/misc/GeoLite2-City-Test.mmdb")?;
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
    let engine = get_engine(None, None, None, vec![])?;

    // dump
    storage.dump_to(&filepath, &engine)?;

    // check metadata
    let metadata = storage.read_metadata(&filepath)?;
    assert!(metadata.is_some());

    // load
    let from_dump = storage.load_from(&filepath)?;

    assert_eq!(
        engine.suggest::<&str>("voronezh", 100, None, None).len(),
        from_dump.suggest::<&str>("voronezh", 100, None, None).len(),
    );

    let coords = (51.6372, 39.1937);
    assert_eq!(
        engine.reverse::<&str>(coords, 1, None, None).unwrap()[0]
            .city
            .id,
        from_dump.reverse::<&str>(coords, 1, None, None).unwrap()[0]
            .city
            .id,
    );

    Ok(())
}

#[test_log::test]
fn population_weight() -> Result<(), Box<dyn Error>> {
    let engine = get_engine(Some("tests/misc/population-weight.txt"), None, None, vec![])?;

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
    let engine = get_engine(None, None, None, vec!["ru", "sr"])?;

    let country1 = engine.country_info("rs").unwrap();
    let country2 = engine.country_info("RS").unwrap();

    assert_eq!(country1.info.geonameid, country2.info.geonameid);
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
