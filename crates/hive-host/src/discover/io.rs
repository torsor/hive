use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::transcript::{adapt, Cursor};

use super::DiscoverError;
use super::TranscriptCursorQuery;

pub fn read_from(path: &Path, offset: u64) -> Result<(Vec<u8>, u64), DiscoverError> {
    let size = path
        .metadata()
        .map_err(|e| DiscoverError(format!("cannot read {}: {e}", path.display())))?
        .len();
    let used = if offset > size { 0 } else { offset };
    let mut f = File::open(path).map_err(|e| DiscoverError(e.to_string()))?;
    f.seek(SeekFrom::Start(used))
        .map_err(|e| DiscoverError(e.to_string()))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| DiscoverError(e.to_string()))?;
    Ok((buf, used))
}

pub fn read_until(path: &Path, end: u64) -> Result<Vec<u8>, DiscoverError> {
    let size = path
        .metadata()
        .map_err(|e| DiscoverError(format!("cannot read {}: {e}", path.display())))?
        .len();
    let stop = end.min(size);
    let mut f = File::open(path).map_err(|e| DiscoverError(e.to_string()))?;
    let mut buf = vec![0u8; stop as usize];
    f.read_exact(&mut buf)
        .map_err(|e| DiscoverError(e.to_string()))?;
    Ok(buf)
}

fn snap_after_newline(path: &Path, offset: u64) -> Result<u64, DiscoverError> {
    if offset == 0 {
        return Ok(0);
    }
    let size = path
        .metadata()
        .map_err(|e| DiscoverError(e.to_string()))?
        .len();
    if offset >= size {
        return Ok(size);
    }
    let mut f = File::open(path).map_err(|e| DiscoverError(e.to_string()))?;
    f.seek(SeekFrom::Start(offset - 1))
        .map_err(|e| DiscoverError(e.to_string()))?;
    let mut prev = [0u8; 1];
    f.read_exact(&mut prev)
        .map_err(|e| DiscoverError(e.to_string()))?;
    if prev[0] == b'\n' {
        return Ok(offset);
    }
    let mut chunk = vec![0u8; (2 * 1024 * 1024).min((size - offset) as usize)];
    let n = f
        .read(&mut chunk)
        .map_err(|e| DiscoverError(e.to_string()))?;
    chunk.truncate(n);
    match chunk.iter().position(|&b| b == b'\n') {
        Some(i) => Ok(offset + i as u64 + 1),
        None => Ok(offset),
    }
}

const TAIL_WINDOW: u64 = 256 * 1024;

pub fn adapt_tail(
    path: &Path,
    session_id: Option<&str>,
    adapter: &str,
    n: usize,
) -> Result<(Vec<crate::transcript::Block>, Cursor, bool, u64), DiscoverError> {
    if n < 1 {
        return Err(DiscoverError("--tail must be a positive integer".into()));
    }
    let size = path
        .metadata()
        .map_err(|e| DiscoverError(e.to_string()))?
        .len();
    if size == 0 {
        return Ok((
            Vec::new(),
            Cursor {
                session_id: session_id.map(str::to_string),
                offset: 0,
            },
            false,
            0,
        ));
    }
    let mut window = size.min(TAIL_WINDOW);
    loop {
        let start_hint = size.saturating_sub(window);
        let start = if start_hint > 0 {
            snap_after_newline(path, start_hint)?
        } else {
            0
        };
        let (data, used) = read_from(path, start)?;
        let (blocks, advanced) = adapt(
            adapter,
            &data,
            Cursor {
                session_id: session_id.map(str::to_string),
                offset: used,
            },
            false,
        )
        .map_err(|e| DiscoverError(e.0))?;
        if blocks.len() >= n || start == 0 {
            return Ok((blocks, advanced, start > 0, start));
        }
        if window >= size {
            return Ok((blocks, advanced, false, 0));
        }
        window = size.min(window.saturating_mul(2));
    }
}

pub fn read_after(
    path: &Path,
    adapter: &str,
    session_id: Option<String>,
    after: Option<&TranscriptCursorQuery>,
    reset: &mut bool,
) -> Result<(Vec<crate::transcript::Block>, Cursor), DiscoverError> {
    let mut offset = after.map(|a| a.offset).unwrap_or(0);
    if let Some(cur) = after {
        if let (Some(a), Some(b)) = (&cur.session_id, &session_id) {
            if a != b {
                *reset = true;
                offset = 0;
            }
        }
    }
    let (data, used) = read_from(path, offset)?;
    if used != offset {
        *reset = true;
    }
    adapt(
        adapter,
        &data,
        Cursor {
            session_id: session_id.clone(),
            offset: used,
        },
        false,
    )
    .map_err(|e| DiscoverError(e.0))
}
