use geosuggest_core::Engine;
use std::{env::temp_dir, error::Error};

fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn get_engine() -> Result<geosuggest_core::Engine, Box<dyn Error>> {
    Engine::new_from_files(
        "tests/misc/cities-ru.txt",
        Some("tests/misc/names.txt"),
        vec!["ru"],
    )
}

#[test]
fn suggest() -> Result<(), Box<dyn Error>> {
    init();
    let engine = get_engine()?;
    let result = engine.suggest("voronezh", 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Voronezh");
    Ok(())
}

#[test]
fn reverse() -> Result<(), Box<dyn Error>> {
    init();
    let engine = get_engine()?;
    let result = engine.reverse((51.6372, 39.1937));
    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "Voronezh");

    Ok(())
}

#[test]
fn build_dump_load() -> Result<(), Box<dyn Error>> {
    init();

    // build
    let engine = get_engine()?;

    // dump
    engine.dump_to_json(temp_dir().join("test-engine.json"))?;

    // load
    let from_dump = Engine::load_from_json(temp_dir().join("test-engine.json"))?;

    assert_eq!(
        engine.suggest("voronezh", 100).len(),
        from_dump.suggest("voronezh", 100).len(),
    );

    let coords = (51.6372, 39.1937);
    assert_eq!(
        engine.reverse(coords).unwrap().id,
        from_dump.reverse(coords).unwrap().id,
    );

    Ok(())
}
