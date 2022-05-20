use getopts::Options;
use std::env;

use geosuggest_core::{Engine, EngineDumpFormat, SourceFileOptions};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "geosuggest_core=info");
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("o", "output", "set output index file name", "INDEX");
    opts.optopt("c", "cities", "set geonames cities file name", "CITIES");
    opts.optopt("n", "names", "set geonames names file name", "NAMES");
    opts.optopt(
        "a",
        "admin1_codes",
        "set geonames admin1 codes file name",
        "ADMIN1_CODES",
    );
    opts.optopt(
        "",
        "countries",
        "set geonames country info file name",
        "COUNTRIES",
    );
    opts.optopt(
        "l",
        "languages",
        "filter names languages comma separated",
        "LANGUAGES",
    );
    opts.optflag("h", "help", "print this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            panic!("{}", f);
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return Ok(());
    }

    let index_file = if let Some(v) = matches.opt_str("o") {
        v
    } else {
        println!("--output option is required");
        print_usage(&program, opts);
        return Ok(());
    };

    let cities_file = if let Some(v) = matches.opt_str("c") {
        v
    } else {
        println!("--cities option is required");
        print_usage(&program, opts);
        return Ok(());
    };

    let names_file = matches.opt_str("n");

    let countries_file = matches.opt_str("countries");

    let admin1_codes_file = matches.opt_str("a");

    let languages_filter = matches
        .opt_str("l")
        .map(|v| {
            v.split(',')
                .map(|i| i.trim().to_owned())
                .collect::<Vec<String>>()
        })
        .unwrap_or_else(|| {
            if names_file.is_some() {
                panic!("Languages must be defined");
            } else {
                Vec::new()
            }
        });

    let engine = Engine::new_from_files(SourceFileOptions {
        cities: &cities_file,
        names: names_file.as_ref(),
        countries: countries_file.as_ref(),
        filter_languages: languages_filter.iter().map(AsRef::as_ref).collect(),
        admin1_codes: admin1_codes_file.as_ref(),
    })
    .unwrap_or_else(|e| {
        panic!(
            "On build index from {} or {:?} - {}",
            &cities_file, &names_file, e
        )
    });
    engine.dump_to(&index_file, EngineDumpFormat::default())?;

    println!("Done. Index file: {}", &index_file);

    Ok(())
}
