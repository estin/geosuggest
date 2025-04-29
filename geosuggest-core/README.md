<div align="center">
  <p><h1>geosuggest-core</h1></p>
  <p><strong>Library to suggest and to find nearest by coordinates cities</strong></p>
  <p></p>
</div>

[Live demo](https://geosuggest.etatarkin.ru/) with [sources](https://github.com/estin/geosuggest/tree/master/geosuggest-demo)

[HTTP service](https://github.com/estin/geosuggest)

[Examples](https://github.com/estin/geosuggest/tree/master/examples/src)

Usage example
```rust
use tokio;
use anyhow::Result;

use geosuggest_core::{Engine, storage::{self, IndexStorage}};
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
    let index_file = std::path::Path::new("/tmp/geosuggest-index.rkyv");

    let updater = IndexUpdater::new(IndexUpdaterSettings {
        names: None, // no multilang support
        ..Default::default()
    })?;

    let storage = storage::Storage::new();

    Ok(if index_file.exists() {
        // load existed index
        let metadata = storage
            .read_metadata(index_file)
            .map_err(|e| anyhow::anyhow!("On load index metadata from {index_file:?}: {e}"))?;

        match metadata {
            Some(m) if updater.has_updates(&m).await? => {
                let engine = updater.build().await?;
                storage
                    .dump_to(index_file, &engine)
                    .map_err(|e| anyhow::anyhow!("Failed dump to {index_file:?}: {e}"))?;
                engine
            }
            _ => storage
                .load_from(index_file)
                .map_err(|e| anyhow::anyhow!("On load index from {index_file:?}: {e}"))?,
        }
    } else {
        // initial
        let engine = updater.build().await?;
        storage
            .dump_to(index_file, &engine)
            .map_err(|e| anyhow::anyhow!("Failed dump to {index_file:?}: {e}"))?;
        engine
    })

}
```
