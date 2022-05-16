use geosuggest_core::{Engine, SourceFileOptions};
use std::{env::temp_dir, error::Error};

#[cfg(feature = "geoip2_support")]
#[cfg(feature = "geoip2_support")]
use std::{net::IpAddr, str::FromStr};

fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn get_engine(
    cities: Option<&str>,
    names: Option<&str>,
    countries: Option<&str>,
) -> Result<geosuggest_core::Engine, Box<dyn Error>> {
    Engine::new_from_files(SourceFileOptions {
        cities: cities.unwrap_or("tests/misc/cities-ru.txt"),
        names: Some(names.unwrap_or("tests/misc/names.txt")),
        countries: Some(countries.unwrap_or("tests/misc/country-info.txt")),
        filter_languages: vec![],
        admin1_codes: Some("tests/misc/admin1-codes.txt"),
    })
}

#[test]
fn suggest() -> Result<(), Box<dyn Error>> {
    init();
    let engine = get_engine(None, None, None)?;
    let items = engine.suggest("voronezh", 1, None);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Voronezh");
    assert_eq!(items[0].country.as_ref().unwrap().name, "Russia");
    assert_eq!(items[0].admin_division.as_ref().unwrap().name, "Voronezj");
    Ok(())
}

#[test]
fn reverse() -> Result<(), Box<dyn Error>> {
    init();
    let engine = get_engine(None, None, None)?;
    let result = engine.reverse((51.6372, 39.1937), 1, None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].city.name, "Voronezh");
    assert_eq!(items[0].city.country.as_ref().unwrap().name, "Russia");
    assert_eq!(
        items[0].city.admin_division.as_ref().unwrap().name,
        "Voronezj"
    );

    Ok(())
}

#[test]
fn capital() -> Result<(), Box<dyn Error>> {
    init();
    let engine = get_engine(None, None, None)?;
    let result = engine.capital("RU");
    assert!(result.is_some());
    let city = result.unwrap();
    assert_eq!(city.name, "Moscow");
    assert_eq!(city.country.as_ref().unwrap().name, "Russia");
    Ok(())
}

#[test]
#[cfg(feature = "geoip2_support")]
fn geoip2_lookup() -> Result<(), Box<dyn Error>> {
    init();
    let mut engine = get_engine(None, None, None)?;
    engine.load_geoip2("tests/misc/GeoLite2-City-Test.mmdb")?;
    let result = engine.geoip2_lookup(IpAddr::from_str("81.2.69.142")?);
    assert!(result.is_some());
    let item = result.unwrap();
    assert_eq!(item.name, "London");

    Ok(())
}

#[test]
#[cfg(feature = "geoip2_support")]
fn geoip2_release_previous_buffer_and_reader() -> Result<(), Box<dyn Error>> {
    init();
    let mut engine = get_engine(None, None, None)?;

    engine.load_geoip2("tests/misc/GeoLite2-City-Test.mmdb")?;

    // Get the resident non-swapped memory of this process that actually takes
    // up space in RAM.
    let memory_before = procinfo::pid::statm_self().unwrap().resident;

    for _ in 0..50 {
        engine.load_geoip2("tests/misc/GeoLite2-City-Test.mmdb")?;
    }

    // let engines: Vec<Engine> = (0..50).map(|_| get_engine(None, None).unwrap()).collect();

    std::thread::sleep(std::time::Duration::from_millis(100));
    let memory_after = procinfo::pid::statm_self().unwrap().resident;

    log::trace!(
        "Memory before: {} after: {} diff: {}",
        memory_before,
        memory_after,
        memory_after - memory_before
    );

    assert_eq!(memory_after - memory_before, 0);

    Ok(())
}

#[test]
fn build_dump_load() -> Result<(), Box<dyn Error>> {
    init();

    // build
    let engine = get_engine(None, None, None)?;

    // dump
    engine.dump_to_json(temp_dir().join("test-engine.json"))?;

    // load
    let from_dump = Engine::load_from_json(temp_dir().join("test-engine.json"))?;

    assert_eq!(
        engine.suggest("voronezh", 100, None).len(),
        from_dump.suggest("voronezh", 100, None).len(),
    );

    let coords = (51.6372, 39.1937);
    assert_eq!(
        engine.reverse(coords, 1, None).unwrap()[0].city.id,
        from_dump.reverse(coords, 1, None).unwrap()[0].city.id,
    );

    Ok(())
}

#[test]
fn population_weight() -> Result<(), Box<dyn Error>> {
    init();

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
    let result = engine.reverse((55.67738, 37.76006), 5, None);
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 3);
    log::trace!("Reverse result: {:#?}", items);
    assert_eq!(items[0].city.name, "Lyublino");

    // with weight coefficient
    let result = engine.reverse((55.67738, 37.76006), 5, Some(population_weight));
    assert!(result.is_some());
    let items = result.unwrap();
    assert_eq!(items.len(), 3);
    log::trace!("Reverse result: {:#?}", items);
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
    let result = engine.reverse((55.67719, 37.89322), 5, Some(population_weight));
    assert!(result.is_some());
    let items = result.unwrap();
    log::trace!("Reverse result: {:#?}", items);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].city.name, "Lyubertsy");

    Ok(())
}
