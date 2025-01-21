use std::{
    fmt::Debug,
    fs::File,
    io::{Seek as _, SeekFrom},
    path::Path,
};

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use tracing::{info, warn};

pub struct FileBacked<T: Serialize + Debug> {
    file: Option<File>,
    dirty: bool,
    cached_value: T,
}

impl<T: DeserializeOwned + Serialize + Debug> FileBacked<T> {
    pub fn new_from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .context("error opening file")?;
        let cached_value =
            serde_json::from_reader(&mut file).context("error deserializing file-backed value")?;
        info!("Loaded cached value from {}", path.display());
        Ok(Self { file: Some(file), dirty: false, cached_value })
    }

    pub fn new_from_file_or(path: impl AsRef<Path>, default: impl FnOnce() -> T) -> Self {
        let path = path.as_ref();
        match File::options().read(true).write(true).create(false).open(path) {
            Ok(mut file) => {
                // file exists, deserialize the value
                match serde_json::from_reader(&mut file) {
                    Ok(cached_value) => {
                        info!("Loaded cached value from {}", path.display());
                        Self { file: Some(file), dirty: false, cached_value }
                    }
                    Err(e) => {
                        warn!("Error deserializing cached value from {}: {}", path.display(), e);
                        Self { file: Some(file), dirty: true, cached_value: default() }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // file doesn't exist, create a new file and use the default value
                info!("No cache file found at {}; creating a new one", path.display());
                let file = File::options()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(path)
                    .inspect_err(|e| {
                        warn!("Error creating cache file: {}", e);
                    })
                    .ok();
                Self { file, dirty: true, cached_value: default() }
            }
            Err(e) => {
                warn!("Error opening cache file {}: {}", path.display(), e);
                Self { file: None, dirty: true, cached_value: default() }
            }
        }
    }
}

impl<T: Serialize + Debug> FileBacked<T> {
    pub fn write_back(&mut self) -> std::io::Result<()> {
        if self.dirty {
            if let Some(file) = &mut self.file {
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                serde_json::to_writer(file, &self.cached_value)?;
                info!("Wrote cached value back to file: {:?}", &self.cached_value);
            }
            self.dirty = false;
        }
        Ok(())
    }

    pub fn get(&self) -> &T {
        &self.cached_value
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.dirty = true;
        &mut self.cached_value
    }

    pub fn into_inner(self) -> T {
        self.cached_value
    }
}

impl<T: DeserializeOwned + Serialize + Debug> FileBacked<T> {
    pub fn quick_read(path: impl AsRef<Path>) -> anyhow::Result<T> {
        let file = File::open(path)?;
        let value: T = serde_json::from_reader(file)?;
        Ok(value)
    }

    pub fn quick_write(path: impl AsRef<Path>, value: &T) -> anyhow::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer(file, value)?;
        Ok(())
    }
}
