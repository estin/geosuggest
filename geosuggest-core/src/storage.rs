use crate::ArchivedEngineMetadata;
use crate::EngineMetadata;
use rkyv;
use rkyv::{deserialize, rancor::Error};
use std::fs::OpenOptions;
use std::io::Read;
use std::io::SeekFrom;
use std::path::Path;

#[cfg(feature = "tracing")]
use std::time::Instant;

/// rkyv storage in len-prefix format `<4-bytes metadata length><metadata><payload>`
pub struct Storage {}

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

impl Storage {
    /// Serialize
    pub fn dump<W>(
        &self,
        buf: &mut W,
        engine_data: &crate::EngineData,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        W: std::io::Write,
    {
        let metadata = rkyv::to_bytes::<Error>(&engine_data.metadata)?;

        buf.write_all(&(metadata.len() as u32).to_be_bytes())?;
        #[cfg(feature = "tracing")]
        buf.write_all(&metadata)?;

        buf.write_all(&engine_data.data)?;
        Ok(())
    }

    /// Deserialize
    pub fn load<R>(&self, buf: &mut R) -> Result<crate::EngineData, Box<dyn std::error::Error>>
    where
        R: std::io::Read + std::io::Seek,
    {
        // skip metadata
        let mut metadata_len = [0; 4];
        buf.read_exact(&mut metadata_len)?;
        let metadata_len = u32::from_be_bytes(metadata_len);
        let _ = buf.seek(SeekFrom::Current(metadata_len as i64))?;

        let mut bytes = rkyv::util::AlignedVec::new();
        bytes.extend_from_reader(buf)?;

        Ok(bytes.try_into().unwrap())
    }

    /// Read engine metadata and don't load whole engine
    pub fn read_metadata<P: AsRef<Path>>(
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
        if metadata_len == 0 {
            return Ok(None);
        }
        let mut raw_metadata = vec![0; metadata_len as usize];
        file.read_exact(&mut raw_metadata)?;

        let archived = rkyv::access::<rkyv::option::ArchivedOption<ArchivedEngineMetadata>, Error>(
            &raw_metadata[..],
        )?;

        Ok(deserialize::<Option<EngineMetadata>, Error>(archived)?)
    }

    /// Dump whole index to file
    pub fn dump_to<P: AsRef<Path>>(
        &self,
        path: P,
        engine_data: &crate::EngineData,
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

        self.dump(&mut file, engine_data)?;

        #[cfg(feature = "tracing")]
        tracing::info!("Dump index to file. took {}ms", now.elapsed().as_millis(),);

        Ok(())
    }
    /// Load whole index from file
    pub fn load_from<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<crate::EngineData, Box<dyn std::error::Error>> {
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
