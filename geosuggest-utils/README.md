<div align="center">
  <p><h1>geosuggest-utils</h1></p>
  <p><strong></strong></p>
  <p></p>
</div>

[HTTP service](https://github.com/estin/geosuggest)

[Examples](https://github.com/estin/geosuggest/tree/master/examples/src)

Usage example
```rust
use tokio;
use geosuggest_utils::{IndexUpdater, IndexUpdaterSettings};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Build index...");
    let updater = IndexUpdater::new(IndexUpdaterSettings {
        names: None, // no multilang support
        ..Default::default()
    })?;

    let engine_data = updater.build().await?;

    let engine = engine_data.as_engine()?;

    println!(
        "Suggest result: {:#?}",
        engine.suggest::<&str>("Beverley", 1, None, Some(&["US"]))
    );
    println!(
        "Reverse result: {:#?}",
        engine.reverse::<&str>((11.138298, 57.510973), 1, None, None)
    );

    Ok(())
}
```
