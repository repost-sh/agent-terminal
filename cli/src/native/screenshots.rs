use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};

use crate::native::render::{self, Cells, Drawing};
use crate::native::store::{self, ScreenshotEntry};
use crate::native::terminal::{Labelled, Terminal};
use crate::protocol::{Ref, Screenshot, ScreenshotDiff, ScreenshotFormat, Shot, request};

struct Drawn {
    frame: u64,
    bytes: Vec<u8>,
    size: Option<(u32, u32)>,
    refs: Option<Vec<Ref>>,
    cells: Option<Cells>,
}

pub(crate) struct Taken {
    pub screenshot: Screenshot,
    pub listed: Option<ScreenshotEntry>,
}

pub(crate) struct Screenshots<'a> {
    pub home: &'a Path,
}

impl Screenshots<'_> {
    pub(crate) fn take(&self, at: &Labelled<'_>, shot: &Shot) -> Result<Taken> {
        let Labelled { session, tab, terminal } = *at;
        let drawn = draw(terminal, shot)?;
        let path =
            self.destination(shot, || format!("{}-{tab}-{}.{}", store::file_name(session), drawn.frame, shot.format));
        store::write_whole(&path, &drawn.bytes)?;
        let listed = drawn.cells.zip(drawn.size).map(|(cells, (width, height))| ScreenshotEntry {
            time: humantime::format_rfc3339_millis(SystemTime::now()).to_string(),
            tab: tab.to_string(),
            frame: drawn.frame,
            path: path.clone(),
            by: None,
            width,
            height,
            cells,
        });
        let screenshot = Screenshot {
            session: session.clone(),
            tab: tab.clone(),
            path,
            format: shot.format,
            frame: drawn.frame,
            width: drawn.size.map(|(width, _)| width),
            height: drawn.size.map(|(_, height)| height),
            refs: drawn.refs,
        };
        Ok(Taken { screenshot, listed })
    }

    pub(crate) fn diff(&self, at: &Labelled<'_>, diff: request::DiffScreenshot) -> Result<ScreenshotDiff> {
        let Labelled { session, tab, terminal } = *at;
        let request::DiffScreenshot { baseline, shot, threshold, .. } = diff;
        let png = std::fs::read(&baseline).with_context(|| format!("read the baseline {}", baseline.display()))?;
        let capture = terminal.capture(shot.rows, false).map_err(|e| anyhow!(e))?;
        let image = render::image(&Drawing { grid: &capture.grid, rows: capture.rows, look: shot.look }, &[])?;
        let compared = render::compare(&png, &image)
            .with_context(|| format!("the baseline {} is not a PNG", baseline.display()))?;
        let path =
            self.destination(&shot, || format!("{}-{tab}-{}-diff.png", store::file_name(session), capture.frame));
        store::write_whole(&path, &compared.image.png()?)?;
        Ok(ScreenshotDiff {
            session: session.clone(),
            tab: tab.clone(),
            baseline,
            path,
            frame: capture.frame,
            changed: compared.changed,
            pixels: compared.pixels,
            threshold,
            passed: f64::from(compared.changed) <= threshold * f64::from(compared.pixels),
        })
    }

    fn destination(&self, shot: &Shot, name: impl FnOnce() -> String) -> PathBuf {
        match (&shot.path, &shot.dir) {
            (Some(path), _) => path.clone(),
            (None, Some(dir)) => dir.join(name()),
            (None, None) => self.home.join("screenshots").join(name()),
        }
    }
}

fn draw(terminal: &Terminal, shot: &Shot) -> Result<Drawn> {
    match shot.format {
        ScreenshotFormat::Txt | ScreenshotFormat::Ansi => text(terminal, shot),
        ScreenshotFormat::Png => png(terminal, shot),
        ScreenshotFormat::Svg => {
            let capture = terminal.capture(shot.rows, shot.annotate).map_err(|e| anyhow!(e))?;
            let refs = capture.refs.as_deref().unwrap_or_default();
            let svg = render::svg(&Drawing { grid: &capture.grid, rows: capture.rows, look: shot.look }, refs)?;
            let size = Some((svg.width, svg.height));
            Ok(Drawn { frame: capture.frame, bytes: svg.text.into_bytes(), size, refs: capture.refs, cells: None })
        }
        ScreenshotFormat::Html => {
            let capture = terminal.capture(shot.rows, false).map_err(|e| anyhow!(e))?;
            let html = render::html(&Drawing { grid: &capture.grid, rows: capture.rows, look: shot.look })?;
            Ok(Drawn { frame: capture.frame, bytes: html.into_bytes(), size: None, refs: None, cells: None })
        }
    }
}

fn text(terminal: &Terminal, shot: &Shot) -> Result<Drawn> {
    let ansi = shot.format == ScreenshotFormat::Ansi;
    let (frame, rows) = terminal.text_rows(shot.rows, ansi).map_err(|e| anyhow!(e))?;
    let text = fmt::from_fn(|f| rows.iter().try_for_each(|row| writeln!(f, "{row}"))).to_string();
    Ok(Drawn { frame, bytes: text.into_bytes(), size: None, refs: None, cells: None })
}

fn png(terminal: &Terminal, shot: &Shot) -> Result<Drawn> {
    let capture = terminal.capture(shot.rows, shot.annotate).map_err(|e| anyhow!(e))?;
    let refs = capture.refs.as_deref().unwrap_or_default();
    let drawing = Drawing { grid: &capture.grid, rows: capture.rows, look: shot.look };
    let image = render::image(&drawing, refs)?;
    let size = Some((image.width(), image.height()));
    let cells = Some(render::cells(&drawing)?);
    Ok(Drawn { frame: capture.frame, bytes: image.png()?, size, refs: capture.refs, cells })
}
