use crate::ArchivedEngineMetadata;
use crate::EngineMetadata;
use rkyv;
use rkyv::{deserialize, option::ArchivedOption, rancor::Error};
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
        let metadata_len = u32::from_be_bytes(metadata_len) as u64;
        let end = buf.seek(SeekFrom::End(0))?;

        // Current layout: payload follows the metadata. Layouts written
        // without `tracing` stored the length prefix but not the metadata
        // itself, so fall back to that offset when the new one holds no
        // valid archive.
        let new_start = 4u64.saturating_add(metadata_len);
        if new_start <= end {
            buf.seek(SeekFrom::Start(new_start))?;
            let mut bytes = rkyv::util::AlignedVec::<128>::new();
            bytes.reserve_exact((end - new_start) as usize);
            bytes.extend_from_reader(buf)?;
            bytes.shrink_to_fit();
            if rkyv::access::<crate::index::ArchivedIndexData, rkyv::rancor::Error>(&bytes)
                .is_ok()
            {
                return Ok(bytes.try_into()?);
            }
        }

        buf.seek(SeekFrom::Start(4))?;
        let mut bytes = rkyv::util::AlignedVec::<128>::new();
        bytes.reserve_exact(end.saturating_sub(4) as usize);
        bytes.extend_from_reader(buf)?;
        bytes.shrink_to_fit();
        rkyv::access::<crate::index::ArchivedIndexData, rkyv::rancor::Error>(&bytes)?;
        Ok(bytes.try_into()?)
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

        let archived =
            rkyv::access::<ArchivedOption<ArchivedEngineMetadata>, Error>(&raw_metadata[..])?;

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

#[cfg(all(test, not(feature = "tracing")))]
mod tests {
    use super::Storage;
    use crate::{EngineData, EngineMetadata};
    use std::io::Cursor;

    #[test]
    fn dump_and_load_preserve_payload_without_tracing() {
        let mut payload = rkyv::util::AlignedVec::<128>::new();
        payload.extend_from_slice(&[0xA5; 256]);
        let mut engine_data = EngineData::try_from(payload).unwrap();
        engine_data.metadata = Some(EngineMetadata::default());

        let storage = Storage::new();
        let mut serialized = Vec::new();
        storage.dump(&mut serialized, &engine_data).unwrap();

        let metadata = rkyv::to_bytes::<rkyv::rancor::Error>(&engine_data.metadata).unwrap();
        let declared_len =
            u32::from_be_bytes(serialized[0..4].try_into().unwrap()) as usize;
        assert_eq!(declared_len, metadata.len());
        assert_eq!(serialized.len(), 4 + metadata.len() + engine_data.data.len());

        let loaded = storage.load(&mut Cursor::new(serialized)).unwrap();
        assert_eq!(loaded.data.as_ref(), engine_data.data.as_ref());
    }
}
