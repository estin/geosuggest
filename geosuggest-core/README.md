<div align="center">
  <p><h1>geosuggest-core</h1></p>
  <p><strong>Library to suggest and to find nearest by coordinates cities</strong></p>
  <p></p>
</div>

[Live demo](https://geosuggest.etatarkin.ru/) with [sources](https://github.com/estin/geosuggest/tree/master/geosuggest-demo)

[Http service](https://github.com/estin/geosuggest)

[Examples](https://github.com/estin/geosuggest/tree/master/examples/src)

Usage example
```rust
use tokio;
use anyhow::Result;

use geosuggest_core::{Engine, EngineDumpFormat};
use geosuggest_utils::{IndexUpdater, IndexUpdaterSettings};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Build index...");
    let engine = load_engine().await?;

    println!(
        "Suggest result: {:#?}",
        engine.suggest::<&str>("Beverley", 1, None, Some(&["us"]))
    );
    println!(
        "Reverse result: {:#?}",
        engine.reverse::<&str>((11.138298, 57.510973), 1, None, None)
    );

    Ok(())
}

async fn load_engine() -> Result<Engine> {
    let index_file = std::path::Path::new("/tmp/geosuggest-index.bincode");

    let updater = IndexUpdater::new(IndexUpdaterSettings {
        names: None, // no multilang support
        ..Default::default()
    })?;

    Ok(if index_file.exists() {
        // load existed index
        let engine = Engine::load_from(index_file, EngineDumpFormat::Bincode)
            .map_err(|e| anyhow::anyhow!("On load index file: {e}"))?;

        if updater.has_updates(&engine).await? {
            // rewrite index file
            let engine = updater.build().await?;
            engine.dump_to(index_file, EngineDumpFormat::Bincode)?;
            engine
        } else {
            engine
        }
    } else {
        // initial
        let engine = updater.build().await?;
        engine.dump_to(index_file, EngineDumpFormat::Bincode)?;
        engine
    })
}
```

