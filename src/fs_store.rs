use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use miette::{IntoDiagnostic, Result, miette};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::NamedTempFile;

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path)
        .into_diagnostic()
        .map_err(|err| err.wrap_err(format!("failed to read {}", path.display())))?;
    serde_json::from_str(&text)
        .into_diagnostic()
        .map_err(|err| err.wrap_err(format!("failed to parse {}", path.display())))
}

pub(crate) fn read_json_value(path: &Path) -> Result<Value> {
    read_json(path)
}

pub(crate) fn write_json<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(data).into_diagnostic()? + "\n";
    write_text(path, &text)
}

pub(crate) fn write_text(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }
    let parent = path
        .parent()
        .ok_or_else(|| miette!("path has no parent: {}", path.display()))?;
    let mut tmp = NamedTempFile::new_in(parent).into_diagnostic()?;
    tmp.write_all(content.as_bytes()).into_diagnostic()?;
    tmp.as_file_mut().sync_all().into_diagnostic()?;
    tmp.persist(path)
        .map_err(|err| miette!("failed to persist {}: {}", path.display(), err.error))?;
    Ok(())
}

pub(crate) fn append_text(path: &Path, content: &str) -> Result<()> {
    let mut existing = String::new();
    if path.exists() {
        File::open(path)
            .into_diagnostic()?
            .read_to_string(&mut existing)
            .into_diagnostic()?;
    }
    existing.push_str(content);
    write_text(path, &existing)
}

pub(crate) fn ensure_file_text(path: &Path, content: &str) -> Result<()> {
    if !path.exists() {
        write_text(path, content)?;
    }
    Ok(())
}

pub(crate) fn ensure_file_json(path: &Path, value: Value) -> Result<()> {
    if !path.exists() {
        write_json(path, &value)?;
    }
    Ok(())
}

pub(crate) fn expand_path(path: &Path) -> Result<PathBuf> {
    let path = if let Some(raw) = path.to_str().filter(|raw| raw.starts_with("~/")) {
        let home = std::env::var("HOME").into_diagnostic()?;
        PathBuf::from(home).join(&raw[2..])
    } else {
        path.to_path_buf()
    };
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir().into_diagnostic()?.join(path))
    }
}
