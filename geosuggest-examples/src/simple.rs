use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use geosuggest_core::{storage, Engine};
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
    tracing::info!("Index metadata: {:#?}", engine.metadata);

    // use
    tracing::info!(
        "Suggest result: {:#?}",
        engine.suggest::<&str>("Beverley", 1, None, Some(&["us"]))
    );
    tracing::info!(
        "Reverse result: {:#?}",
        engine.reverse::<&str>((11.138298, 57.510973), 1, None, None)
    );
    tracing::info!("Country info: {:#?}", engine.country_info("RS"));
    tracing::info!("Capital info: {:#?}", engine.capital("GB"));

    Ok(())
}

async fn load_engine() -> Result<Engine> {
    let index_file = std::path::Path::new("/tmp/geosuggest-index.rkyv");

    let updater = IndexUpdater::new(IndexUpdaterSettings {
        filter_languages: vec!["ru", "ar"],
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
