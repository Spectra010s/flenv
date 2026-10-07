use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

/// Download a URL to `destination`, resuming a `.part` file when present.
/// A failed or interrupted transfer keeps the `.part` file for retry and
/// never leaves a truncated file at the final path.
pub fn download(url: &str, destination: &Path, component: &str) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let part = destination.with_extension("part");
    let resumed = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    let mut request = ureq::get(url);
    if resumed > 0 {
        request = request.header("Range", &format!("bytes={resumed}-"));
    }
    let mut response = request
        .call()
        .with_context(|| format!("{component} download failed; partial file kept for retry"))?;

    // Server ignored the range: restart from scratch rather than appending.
    let status = response.status();
    let mut out = if status == 206 {
        File::options()
            .append(true)
            .open(&part)
            .with_context(|| format!("cannot append to {}", part.display()))?
    } else {
        File::create(&part).with_context(|| format!("cannot write to {}", part.display()))?
    };
    io::copy(&mut response.body_mut().as_reader(), &mut out).with_context(|| {
        format!(
            "{component} download failed; partial file kept at {}",
            part.display()
        )
    })?;
    out.flush()?;
    drop(out);

    fs::rename(&part, destination).with_context(|| {
        format!(
            "cannot finalize {component} download ({} -> {})",
            part.display(),
            destination.display()
        )
    })?;
    Ok(())
}

/// Fetch a URL body as text (for small metadata documents, not SDKs).
pub fn fetch_string(url: &str, component: &str) -> Result<String> {
    let mut response = ureq::get(url)
        .call()
        .with_context(|| format!("{component} download failed"))?;
    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .read_to_string(&mut body)
        .context("cannot read response")?;
    Ok(body)
}
