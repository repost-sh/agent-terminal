use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{Name, Ref, RowRange};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Theme {
    #[default]
    Dark,
    Light,
}

impl FromStr for Theme {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            _ => Err(format!("unknown theme {text}; use dark or light")),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScreenshotFormat {
    #[default]
    Png,
    Svg,
    Html,
    Txt,
    Ansi,
}

impl ScreenshotFormat {
    pub(crate) const ALL: [ScreenshotFormat; 5] = [
        ScreenshotFormat::Png,
        ScreenshotFormat::Svg,
        ScreenshotFormat::Html,
        ScreenshotFormat::Txt,
        ScreenshotFormat::Ansi,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            ScreenshotFormat::Png => "png",
            ScreenshotFormat::Svg => "svg",
            ScreenshotFormat::Html => "html",
            ScreenshotFormat::Txt => "txt",
            ScreenshotFormat::Ansi => "ansi",
        }
    }
}

impl FromStr for ScreenshotFormat {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let lower = text.to_ascii_lowercase();
        ScreenshotFormat::ALL
            .into_iter()
            .find(|format| format.name() == lower)
            .ok_or_else(|| format!("unknown format {text}; use png, svg, html, txt or ansi"))
    }
}

impl fmt::Display for ScreenshotFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub(crate) struct Look {
    pub scale: u8,
    pub theme: Theme,
    pub cursor: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self { scale: 1, theme: Theme::Dark, cursor: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub(crate) struct Shot {
    pub path: Option<PathBuf>,
    pub dir: Option<PathBuf>,
    pub format: ScreenshotFormat,
    pub look: Look,
    pub rows: Option<RowRange>,
    pub annotate: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ScreenshotDiff {
    pub session: Name,
    pub tab: Name,
    pub baseline: PathBuf,
    pub path: PathBuf,
    pub frame: u64,
    pub changed: u32,
    pub pixels: u32,
    pub threshold: f64,
    pub passed: bool,
}

impl ScreenshotDiff {
    pub(crate) fn summary(&self) -> String {
        let share = if self.pixels == 0 { 0.0 } else { f64::from(self.changed) * 100.0 / f64::from(self.pixels) };
        let verdict = if self.passed { "within" } else { "above" };
        format!(
            "{} of {} pixels ({share:.2}%) differ from {}, {verdict} the threshold {}; diff image {}",
            self.changed,
            self.pixels,
            self.baseline.display(),
            self.threshold,
            self.path.display()
        )
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Screenshot {
    pub session: Name,
    pub tab: Name,
    pub path: PathBuf,
    pub format: ScreenshotFormat,
    pub frame: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<Ref>>,
}
