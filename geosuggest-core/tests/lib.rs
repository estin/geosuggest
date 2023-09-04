use geosuggest_core::{Engine, EngineDumpFormat, SourceFileOptions};
use std::collections::HashMap;
use std::{env::temp_dir, error::Error};

#[cfg(feature = "geoip2_support")]
#[cfg(feature = "geoip2_support")]
use std::{net::IpAddr, str::FromStr};

fn get_engine(
    cities: Option<&str>,
    names: Option<&str>,
    countries: Option<&str>,
) -> Result<geosuggest_core::Engine, Box<dyn Error>> {
    Engine::new_from_files(
        SourceFileOptions {
            cities: cities.unwrap_or("tests/misc/cities-ru.txt"),
            names: Some(names.unwrap_or("tests/misc/names.txt")),
            countries: Some(countries.unwrap_or("tests/misc/country-info.txt")),
            filter_languages: vec![],
            admin1_codes: Some("tests/misc/admin1-codes.txt"),
            admin2_codes: Some("tests/misc/admin2-codes.txt"),
        },
        HashMap::new(),
    )
}

#[test_log::test]
fn suggest() -> Result<(), Box<dyn Error>> {
    let engine = get_engine(None, None, None)?;

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
    let engine = get_engine(None, None, None)?;
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
    let engine = get_engine(None, None, None)?;
    let result = engine.capital("RU");
    assert!(result.is_some());
    let city = result.unwrap();
    assert_eq!(city.name, "Moscow");
    assert_eq!(city.country.as_ref().unwrap().name, "Russia");
    Ok(())
}

#[test_log::test]
#[cfg(feature = "geoip2_support")]
fn geoip2_lookup() -> Result<(), Box<dyn Error>> {
    let mut engine = get_engine(None, None, None)?;
    engine.load_geoip2("tests/misc/GeoLite2-City-Test.mmdb")?;
    let result = engine.geoip2_lookup(IpAddr::from_str("81.2.69.142")?);
    assert!(result.is_some());
    let item = result.unwrap();
    assert_eq!(item.name, "London");

    Ok(())
}

#[test_log::test]
fn build_dump_load() -> Result<(), Box<dyn Error>> {
    // build
    let engine = get_engine(None, None, None)?;

    // dump
    engine.dump_to(
        temp_dir().join("test-engine.json"),
        EngineDumpFormat::default(),
    )?;

    // load
    let from_dump = Engine::load_from(
        temp_dir().join("test-engine.json"),
        EngineDumpFormat::default(),
    )?;

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
    let engine = get_engine(Some("tests/misc/population-weight.txt"), None, None)?;

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
