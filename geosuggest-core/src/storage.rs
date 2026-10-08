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

// Metadata is tiny; anything bigger is corruption, and reading it unchecked
// could exhaust memory.
const MAX_METADATA_LEN: u64 = 1 << 20;

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
        // always stamp the current layout version, even when no metadata
        // was set, so `load` can reject stale files with a clear error
        let mut stamped = engine_data.metadata.clone().unwrap_or_default();
        stamped.index_format_version = crate::index::INDEX_FORMAT_VERSION;
        let metadata = rkyv::to_bytes::<Error>(&Some(stamped))?;
        buf.write_all(&(metadata.len() as u32).to_be_bytes())?;
        buf.write_all(&metadata)?;

        buf.write_all(&engine_data.data)?;
        Ok(())
    }

    /// Deserialize. Rejects index files from other layout versions with a
    /// rebuild request instead of misreading them.
    pub fn load<R>(&self, buf: &mut R) -> Result<crate::EngineData, Box<dyn std::error::Error>>
    where
        R: std::io::Read + std::io::Seek,
    {
        fn stale(msg: impl Into<String>) -> Box<dyn std::error::Error> {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{}; rebuild the index and retry", msg.into()),
            ))
        }
        // metadata is tiny; anything bigger is corruption, and reading it
        // unchecked could exhaust memory (see MAX_METADATA_LEN)
        let mut len_prefix = [0; 4];
        if buf.read_exact(&mut len_prefix).is_err() {
            return Err(stale("cannot read index header"));
        }
        let metadata_len = u32::from_be_bytes(len_prefix) as u64;
        if metadata_len == 0 || metadata_len > MAX_METADATA_LEN {
            return Err(stale("index header is corrupt"));
        }
        let end = buf.seek(SeekFrom::End(0)).map_err(|_| stale("cannot read index"))?;
        if end.saturating_sub(4) < metadata_len {
            return Err(stale("index file is truncated"));
        }
        buf.seek(SeekFrom::Start(4))
            .map_err(|_| stale("cannot read index"))?;
        let mut raw_metadata = vec![0u8; metadata_len as usize];
        if buf.read_exact(&mut raw_metadata).is_err() {
            return Err(stale("index file is truncated"));
        }
        let metadata: Option<EngineMetadata> =
            match rkyv::from_bytes::<Option<EngineMetadata>, rkyv::rancor::Error>(&raw_metadata)
            {
                Ok(metadata) => metadata,
                Err(_) => return Err(stale("index metadata is unreadable")),
            };
        match metadata {
            Some(ref m) if m.index_format_version == crate::index::INDEX_FORMAT_VERSION => {}
            Some(ref m) => {
                return Err(stale(format!(
                    "index format version {} is not supported (code expects {})",
                    m.index_format_version,
                    crate::index::INDEX_FORMAT_VERSION
                )))
            }
            None => return Err(stale("index has no format version")),
        }

        // reserve the exact payload size so the buffer never rounds up to a
        // power of two while reading a large index
        let payload_start = 4 + metadata_len;
        let mut bytes = rkyv::util::AlignedVec::<128>::new();
        bytes.reserve_exact(end.saturating_sub(payload_start) as usize);
        buf.seek(SeekFrom::Start(payload_start))
            .map_err(|_| stale("cannot read index"))?;
        if bytes.extend_from_reader(buf).is_err() {
            return Err(stale("index payload is truncated"));
        }
        bytes.shrink_to_fit();

        Ok(bytes.try_into()?)
    }

    /// Read engine metadata and don't load whole engine.
    ///
    /// Returns `Ok(None)` when the file holds no usable metadata: an empty
    /// header, or bytes from an older layout that no longer parse. Callers
    /// that need a current index should rebuild in that case; `load` itself
    /// rejects such files with a rebuild request.
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
        if metadata_len == 0 || u64::from(metadata_len) > MAX_METADATA_LEN {
            return Ok(None);
        }
        let mut raw_metadata = vec![0; metadata_len as usize];
        file.read_exact(&mut raw_metadata)?;

        let archived =
            match rkyv::access::<ArchivedOption<ArchivedEngineMetadata>, Error>(&raw_metadata[..])
            {
                Ok(archived) => archived,
                Err(_) => return Ok(None),
            };

        Ok(deserialize::<Option<EngineMetadata>, Error>(archived).unwrap_or(None))
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

        let mut stamped = engine_data.metadata.clone().unwrap_or_default();
        stamped.index_format_version = crate::index::INDEX_FORMAT_VERSION;
        let metadata = rkyv::to_bytes::<rkyv::rancor::Error>(&Some(stamped)).unwrap();
        let declared_len =
            u32::from_be_bytes(serialized[0..4].try_into().unwrap()) as usize;
        assert_eq!(declared_len, metadata.len());
        assert_eq!(serialized.len(), 4 + metadata.len() + engine_data.data.len());

        let loaded = storage.load(&mut Cursor::new(serialized)).unwrap();
        assert_eq!(loaded.data.as_ref(), engine_data.data.as_ref());
    }
}
