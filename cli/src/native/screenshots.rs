use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};

use crate::connection::home;
use crate::native::render::{self, Cells};
use crate::native::store::{self, ScreenshotEntry};
use crate::native::terminal::Terminal;
use crate::protocol::{Ref, Screenshot, ScreenshotDiff, ScreenshotFormat, Shot};

struct Drawn {
    frame: u64,
    bytes: Vec<u8>,
    size: Option<(u32, u32)>,
    refs: Option<Vec<Ref>>,
    cells: Option<Cells>,
}

pub struct Taken {
    pub screenshot: Screenshot,
    pub listed: Option<ScreenshotEntry>,
}

pub fn take(session: &str, tab: &str, terminal: &Terminal, shot: &Shot, by: Option<String>) -> Result<Taken> {
    let drawn = match shot.format {
        ScreenshotFormat::Txt | ScreenshotFormat::Ansi => {
            let ansi = shot.format == ScreenshotFormat::Ansi;
            let (frame, rows) = terminal.text_rows(shot.rows, ansi).map_err(|e| anyhow!(e))?;
            let text: String = rows.iter().map(|row| format!("{row}\n")).collect();
            Drawn { frame, bytes: text.into_bytes(), size: None, refs: None, cells: None }
        }
        ScreenshotFormat::Png => {
            let capture = terminal.capture(shot.rows, shot.annotate).map_err(|e| anyhow!(e))?;
            let refs = capture.refs.as_deref().unwrap_or_default();
            let image = render::image(&capture.grid, capture.rows, &shot.look, refs);
            let size = Some((image.width(), image.height()));
            let cells = Some(render::cells(&capture.grid, capture.rows, &shot.look));
            Drawn { frame: capture.frame, bytes: image.png()?, size, refs: capture.refs, cells }
        }
        ScreenshotFormat::Svg => {
            let capture = terminal.capture(shot.rows, shot.annotate).map_err(|e| anyhow!(e))?;
            let refs = capture.refs.as_deref().unwrap_or_default();
            let svg = render::svg(&capture.grid, capture.rows, &shot.look, refs);
            let size = Some((svg.width, svg.height));
            Drawn { frame: capture.frame, bytes: svg.text.into_bytes(), size, refs: capture.refs, cells: None }
        }
        ScreenshotFormat::Html => {
            let capture = terminal.capture(shot.rows, false).map_err(|e| anyhow!(e))?;
            let html = render::html(&capture.grid, capture.rows, &shot.look);
            Drawn { frame: capture.frame, bytes: html.into_bytes(), size: None, refs: None, cells: None }
        }
    };
    let path = match &shot.path {
        Some(path) => path.clone(),
        None => {
            let name = format!("{}-{tab}-{}.{}", store::file_name(session), drawn.frame, shot.format);
            folder(shot.dir.as_deref())?.join(name)
        }
    };
    store::write_whole(&path, &drawn.bytes)?;
    let listed = drawn.cells.zip(drawn.size).map(|(cells, (width, height))| ScreenshotEntry {
        time: humantime::format_rfc3339_millis(SystemTime::now()).to_string(),
        tab: tab.to_string(),
        frame: drawn.frame,
        path: path.clone(),
        by,
        width,
        height,
        cells,
    });
    let screenshot = Screenshot {
        session: session.to_string(),
        tab: tab.to_string(),
        path,
        format: shot.format,
        frame: drawn.frame,
        width: drawn.size.map(|(width, _)| width),
        height: drawn.size.map(|(_, height)| height),
        refs: drawn.refs,
    };
    Ok(Taken { screenshot, listed })
}

pub fn diff(
    session: &str,
    tab: &str,
    terminal: &Terminal,
    baseline: PathBuf,
    shot: &Shot,
    threshold: f64,
) -> Result<ScreenshotDiff> {
    let png = std::fs::read(&baseline).with_context(|| format!("read the baseline {}", baseline.display()))?;
    let capture = terminal.capture(shot.rows, false).map_err(|e| anyhow!(e))?;
    let image = render::image(&capture.grid, capture.rows, &shot.look, &[]);
    let compared =
        render::compare(&png, &image).with_context(|| format!("the baseline {} is not a PNG", baseline.display()))?;
    let path = match &shot.path {
        Some(path) => path.clone(),
        None => {
            let name = format!("{}-{tab}-{}-diff.png", store::file_name(session), capture.frame);
            folder(shot.dir.as_deref())?.join(name)
        }
    };
    store::write_whole(&path, &compared.image.png()?)?;
    Ok(ScreenshotDiff {
        session: session.to_string(),
        tab: tab.to_string(),
        baseline,
        path,
        frame: capture.frame,
        changed: compared.changed,
        pixels: compared.pixels,
        threshold,
        passed: compared.changed as f64 <= threshold * compared.pixels as f64,
    })
}

pub fn folder(dir: Option<&Path>) -> Result<PathBuf> {
    match dir {
        Some(dir) => Ok(dir.to_path_buf()),
        None => Ok(home()?.join("screenshots")),
    }
}
