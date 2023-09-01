use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use geosuggest_core::{Engine, EngineDumpFormat};
use geosuggest_utils::{IndexUpdater, IndexUpdaterSettings};

#[tokio::main]
async fn main() -> Result<()> {
    // logging
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer());
    subscriber.init();

    // build/load/update index
    let engine = load_engine().await?;

    // use
    tracing::info!(
        "Suggest result: {:#?}",
        engine.suggest::<&str>("Beverley", 1, None, Some(&["us"]))
    );
    tracing::info!(
        "Reverse result: {:#?}",
        engine.reverse::<&str>((11.138298, 57.510973), 1, None, None)
    );

    Ok(())
}

async fn load_engine() -> Result<Engine> {
    let index_file = std::path::Path::new("/tmp/geosuggest-index.bincode");

    let updater = IndexUpdater::new(IndexUpdaterSettings {
        filter_languages: vec!["ru", "ar"],
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
