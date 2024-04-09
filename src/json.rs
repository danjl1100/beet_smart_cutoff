use crate::JsonMap;
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

pub struct JsonFile {
    pub map: Option<JsonMap>,
    pub path: PathBuf,
}
pub fn read_json_file(path: PathBuf) -> anyhow::Result<JsonFile> {
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(JsonFile { map: None, path })
        }
        Err(e) => Err(e)?,
    };
    let file = BufReader::new(file);
    let value: serde_json::Value = serde_json::from_reader(file)?;

    let serde_json::Value::Object(map) = value else {
        anyhow::bail!("unexpected JSON value: {value:?}")
    };

    let entry_count = map.len();
    let filename = path.display();
    println!("Loaded {entry_count} entries from {filename}");

    Ok(JsonFile {
        map: Some(map),
        path,
    })
}
pub fn write_json_file(path: impl AsRef<Path>, value: JsonMap) -> anyhow::Result<()> {
    let file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &value)?;

    let entry_count = value.len();
    let filename = path.as_ref().display();
    println!("Saved {entry_count} entries to {filename}");

    Ok(())
}
