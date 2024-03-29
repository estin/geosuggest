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
use anyhow::Result;

use geosuggest_utils::{IndexUpdater, IndexUpdaterSettings};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Build index...");
    let updater = IndexUpdater::new(IndexUpdaterSettings {
        names: None, // no multilang support
        ..Default::default()
    })?;

    let engine = updater.build().await?;

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
```
