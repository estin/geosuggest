use crate::{Engine, EngineMetadata};
use std::fs::OpenOptions;
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

#[cfg(feature = "serde_json")]
pub mod json {
    use super::IndexStorage;
    use crate::{Engine, EngineDump, EngineMetadata};
    pub use serde::{Deserialize, Serialize};
    use std::fs::OpenOptions;
    use std::io::BufRead;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::Path;

    /// JSON storage in 2-lines format `<metadata>\n<payload>`
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
            serde_json::to_writer(buff.by_ref(), &engine.metadata)?;
            writeln!(buff.by_ref())?;
            serde_json::to_writer(buff, &engine)?;
            Ok(())
        }
        /// Deserialize engine
        fn load<R>(&self, buff: &mut R) -> Result<Engine, Box<dyn std::error::Error>>
        where
            R: std::io::Read + std::io::Seek,
        {
            let Some(raw_payload) = std::io::BufReader::new(buff).lines().nth(1) else {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidData).into());
            };

            Ok(serde_json::from_str::<EngineDump>(&raw_payload?)?.into())
        }
        /// Read engine metadata and don't load whole engine
        fn read_metadata<P: AsRef<Path>>(
            &self,
            path: P,
        ) -> Result<Option<EngineMetadata>, Box<dyn std::error::Error>> {
            let file = OpenOptions::new()
                .create(false)
                .read(true)
                .truncate(false)
                .open(&path)?;

            let Some(raw_metadata) = std::io::BufReader::new(file).lines().next() else {
                return Ok(None);
            };

            Ok(Some(serde_json::from_str(&raw_metadata?)?))
        }
    }
}

#[cfg(feature = "bincode")]
pub mod bincode {
    use super::IndexStorage;
    use crate::{Engine, EngineDump, EngineMetadata};
    pub use serde::{Deserialize, Serialize};
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::io::{Seek, SeekFrom};
    use std::path::Path;

    /// Bincode storage in len-prefix format `<4-bytes metadata length><metadata><payload>`
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
            let config = bincode::config::standard();
            let metadata = bincode::serde::encode_to_vec(&engine.metadata, config)?;
            buff.write_all(&(metadata.len() as u32).to_be_bytes())?;
            buff.write_all(&metadata)?;
            bincode::serde::encode_into_std_write(engine, buff, config)?;
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
            // // TODO use Seek?
            // // std::io::copy(buff.take(metadata_len.into()), &mut std::io::sink());
            let mut skip = vec![0; metadata_len as usize];
            buff.read_exact(&mut skip)?;

            // buff.seek(SeekFrom::Start(metadata_len as u64))?;

            // load payload
            Ok(bincode::serde::decode_from_std_read::<EngineDump, _, _>(
                buff,
                bincode::config::standard(),
            )?
            .into())
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

            let (metadata, _) = bincode::serde::borrow_decode_from_slice(
                &raw_metadata,
                bincode::config::standard(),
            )?;

            Ok(metadata)
        }
    }
}

#[cfg(feature = "rkyv")]
pub mod rkyv {
    use super::IndexStorage;
    use crate::{ArchivedEngineDump, ArchivedEngineMetadata, Engine, EngineDump, EngineMetadata};
    use rkyv;
    use rkyv::ArchivedMetadata;
    pub use rkyv::{deserialize, rancor::Error, with, Archive, Deserialize, Serialize};
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::io::{Seek, SeekFrom};
    use std::path::Path;

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
            // // TODO use Seek?
            // // std::io::copy(buff.take(metadata_len.into()), &mut std::io::sink());
            let mut skip = vec![0; metadata_len as usize];
            buff.read_exact(&mut skip)?;
            // buff.seek(SeekFrom::End(metadata_len as i64))?;

            // Read all bytes into memory (for small data)
            let mut bytes = Vec::new();
            buff.read_to_end(&mut bytes)?;

            // Validate and deserialize
            let archived = rkyv::access::<ArchivedEngineDump, Error>(&bytes[..]).unwrap();

            Ok(deserialize::<EngineDump, Error>(archived).unwrap().into())
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

            let archived =
                rkyv::access::<ArchivedEngineMetadata, Error>(&raw_metadata[..]).unwrap();

            Ok(deserialize::<EngineMetadata, Error>(archived)
                .unwrap()
                .into())
        }
    }
}

#[cfg(feature = "serde_json")]
pub use self::json::*;

#[cfg(feature = "bincode")]
pub use self::bincode::*;

#[cfg(feature = "rkyv")]
pub use self::rkyv::*;
