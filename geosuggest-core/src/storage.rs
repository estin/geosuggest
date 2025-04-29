use crate::{ArchivedEngineDump, ArchivedEngineMetadata, EngineDump};
use crate::{Engine, EngineMetadata};
use rkyv;
use rkyv::{deserialize, rancor::Error};
use std::fs::OpenOptions;
use std::io::Read;
use std::io::SeekFrom;
use std::path::Path;

#[cfg(feature = "tracing")]
use std::time::Instant;

pub trait IndexStorage {
    /// Serialize engine
    fn dump<W>(&self, engine: &Engine, buff: &mut W) -> Result<(), Box<dyn std::error::Error>>
    where
        W: std::io::Write;
    /// Deserialize engine
    fn load<R>(&self, buff: &mut R) -> Result<Engine, Box<dyn std::error::Error>>
    where
        R: std::io::Read + std::io::Seek;
    /// Read engine metadata (don't load whole engine)
    fn read_metadata<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Option<EngineMetadata>, Box<dyn std::error::Error>>;
    /// Dump whole engine to file
    fn dump_to<P: AsRef<Path>>(
        &self,
        path: P,
        engine: &Engine,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(feature = "tracing")]
        tracing::info!("Start dump index to file...");
        #[cfg(feature = "tracing")]
        let now = Instant::now();

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;

        self.dump(engine, &mut file)?;

        #[cfg(feature = "tracing")]
        tracing::info!("Dump index to file. took {}ms", now.elapsed().as_millis(),);

        Ok(())
    }
    /// Load whole engine from file
    fn load_from<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<Engine, Box<dyn std::error::Error>> {
        #[cfg(feature = "tracing")]
        tracing::info!("Loading index...");
        #[cfg(feature = "tracing")]
        let now = Instant::now();

        let mut file = OpenOptions::new()
            .create(false)
            .read(true)
            .truncate(false)
            .open(&path)?;

        let index = self.load(&mut file)?;

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Loaded from file done. took {}ms",
            now.elapsed().as_millis(),
        );

        Ok(index)
    }
}

/// rkyv storage in len-prefix format `<4-bytes metadata length><metadata><payload>`
pub struct Storage;

impl Storage {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexStorage for Storage {
    /// Serialize engine
    fn dump<W>(&self, engine: &Engine, buff: &mut W) -> Result<(), Box<dyn std::error::Error>>
    where
        W: std::io::Write,
    {
        let metadata = rkyv::to_bytes::<Error>(&engine.metadata).unwrap();
        buff.write_all(&(metadata.len() as u32).to_be_bytes())?;
        buff.write_all(&metadata)?;
        let data = rkyv::to_bytes::<Error>(engine).unwrap();
        buff.write_all(&data)?;
        Ok(())
    }

    /// Deserialize engine
    fn load<R>(&self, buff: &mut R) -> Result<Engine, Box<dyn std::error::Error>>
    where
        R: std::io::Read + std::io::Seek,
    {
        // skip metadata
        let mut metadata_len = [0; 4];
        buff.read_exact(&mut metadata_len)?;
        let metadata_len = u32::from_be_bytes(metadata_len);
        let pos = buff.seek(SeekFrom::Current(metadata_len as i64))?;
        println!("New pos is {pos} where metadata len is {metadata_len}");

        // Read all bytes into memory (for small data)
        let mut bytes = Vec::new();
        buff.read_to_end(&mut bytes)?;

        // Validate and deserialize
        let archived = rkyv::access::<ArchivedEngineDump, Error>(&bytes[..])?;

        Ok(deserialize::<EngineDump, Error>(archived)?.into())
    }

    /// Read engine metadata and don't load whole engine
    fn read_metadata<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Option<EngineMetadata>, Box<dyn std::error::Error>> {
        let mut file = OpenOptions::new()
            .create(false)
            .read(true)
            .truncate(false)
            .open(&path)?;

        let mut metadata_len = [0; 4];
        file.read_exact(&mut metadata_len)?;

        let metadata_len = u32::from_be_bytes(metadata_len);
        let mut raw_metadata = vec![0; metadata_len as usize];
        file.read_exact(&mut raw_metadata)?;

        let archived = rkyv::access::<ArchivedEngineMetadata, Error>(&raw_metadata[..])?;

        Ok(deserialize::<EngineMetadata, Error>(archived)?.into())
    }
}
