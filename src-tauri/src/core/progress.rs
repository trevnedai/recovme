// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
//! Reads live progress from the engines' output files.

use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeepProgress {
    pub pass: u32,
    pub sector: u64,
    pub total: u64,
    pub files: u64,
    pub eta_secs: Option<u64>,
}

/// Last `max` bytes of a file as text (engines rewrite the screen, so the tail is current).
pub fn tail(path: &Path, max: u64) -> String {
    let Ok(mut f) = File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len > max {
        let _ = f.seek(SeekFrom::Start(len - max));
    }
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).to_string()
}

/// Parse PhotoRec's screen output:
/// "Pass 1 - Reading sector     216576/3072000, 8 files found"
/// "Estimated time to completion 0h00m13"
pub fn parse_deep(text: &str) -> Option<DeepProgress> {
    static RE: OnceLock<Regex> = OnceLock::new();
    static ETA: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"Pass (\d+) - Reading sector\s+(\d+)/(\d+), (\d+) files? found").unwrap()
    });
    let eta = ETA.get_or_init(|| Regex::new(r"Estimated time to completion (\d+)h(\d+)m(\d+)").unwrap());
    let c = re.captures_iter(text).last()?;
    let mut p = DeepProgress {
        pass: c[1].parse().ok()?,
        sector: c[2].parse().ok()?,
        total: c[3].parse().ok()?,
        files: c[4].parse().ok()?,
        eta_secs: None,
    };
    if let Some(e) = eta.captures_iter(text).last() {
        let h: u64 = e[1].parse().unwrap_or(0);
        let m: u64 = e[2].parse().unwrap_or(0);
        let s: u64 = e[3].parse().unwrap_or(0);
        p.eta_secs = Some(h * 3600 + m * 60 + s);
    }
    Some(p)
}

/// Minimal terminal emulator: PhotoRec draws its progress with ncurses, which
/// only rewrites the characters that changed. Replaying the escape codes onto a
/// virtual screen gives back the current text, e.g. "Pass 1 - Reading sector …".
pub struct Screen {
    rows: Vec<Vec<char>>,
    r: usize,
    c: usize,
    saved: (usize, usize),
    top: usize,
    bottom: usize,
    state: EscState,
    params: String,
    offset: u64,
}

#[derive(PartialEq)]
enum EscState {
    Normal,
    Esc,
    Csi,
    Charset,
}

const ROWS: usize = 60;
const COLS: usize = 240;

impl Default for Screen {
    fn default() -> Self {
        Screen {
            rows: vec![vec![' '; COLS]; ROWS],
            r: 0,
            c: 0,
            saved: (0, 0),
            top: 0,
            bottom: ROWS - 1,
            state: EscState::Normal,
            params: String::new(),
            offset: 0,
        }
    }
}

impl Screen {
    /// Read whatever the engine wrote since the last call.
    pub fn feed_file(&mut self, path: &Path) {
        let Ok(mut f) = File::open(path) else { return };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            *self = Screen::default();
        }
        if len == self.offset || f.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_ok() {
            self.offset += buf.len() as u64;
            self.feed(&buf);
        }
    }

    pub fn text(&self) -> String {
        self.rows
            .iter()
            .map(|r| r.iter().collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn nums(&self) -> Vec<usize> {
        self.params
            .trim_start_matches(['?', '>', '='])
            .split(';')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    }

    fn scroll_up(&mut self) {
        let (t, b) = (self.top, self.bottom.min(ROWS - 1));
        if t < b {
            self.rows[t..=b].rotate_left(1);
            self.rows[b] = vec![' '; COLS];
        }
    }

    fn scroll_down(&mut self) {
        let (t, b) = (self.top, self.bottom.min(ROWS - 1));
        if t < b {
            self.rows[t..=b].rotate_right(1);
            self.rows[t] = vec![' '; COLS];
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            match self.state {
                EscState::Normal => match b {
                    0x1b => self.state = EscState::Esc,
                    b'\r' => self.c = 0,
                    b'\n' => {
                        if self.r == self.bottom {
                            self.scroll_up();
                        } else if self.r < ROWS - 1 {
                            self.r += 1;
                        }
                    }
                    0x08 => self.c = self.c.saturating_sub(1),
                    b'\t' => self.c = ((self.c / 8) + 1) * 8,
                    0x20..=0x7e | 0x80..=0xff => {
                        if self.c < COLS {
                            self.rows[self.r][self.c] = b as char;
                        }
                        self.c += 1;
                    }
                    _ => {}
                },
                EscState::Esc => {
                    self.state = EscState::Normal;
                    match b {
                        b'[' => {
                            self.state = EscState::Csi;
                            self.params.clear();
                        }
                        b'(' | b')' | b'*' | b'+' => self.state = EscState::Charset,
                        b'M' => {
                            if self.r == self.top {
                                self.scroll_down();
                            } else {
                                self.r = self.r.saturating_sub(1);
                            }
                        }
                        b'D' => {
                            if self.r == self.bottom {
                                self.scroll_up();
                            } else {
                                self.r = (self.r + 1).min(ROWS - 1);
                            }
                        }
                        b'7' => self.saved = (self.r, self.c),
                        b'8' => (self.r, self.c) = self.saved,
                        b'c' => *self = Screen { offset: self.offset, ..Screen::default() },
                        _ => {}
                    }
                }
                EscState::Charset => self.state = EscState::Normal,
                EscState::Csi => {
                    if (0x40..=0x7e).contains(&b) {
                        self.state = EscState::Normal;
                        self.csi(b);
                    } else {
                        self.params.push(b as char);
                        if self.params.len() > 32 {
                            self.state = EscState::Normal;
                        }
                    }
                }
            }
        }
    }

    fn csi(&mut self, fin: u8) {
        if self.params.starts_with('?') && fin != b'J' && fin != b'K' {
            return; // private modes (cursor visibility, alt screen …)
        }
        let n = self.nums();
        let p = |i: usize, d: usize| -> usize {
            match n.get(i) {
                Some(&v) if v > 0 => v,
                _ => d,
            }
        };
        let clamp_r = |v: usize| v.min(ROWS - 1);
        let clamp_c = |v: usize| v.min(COLS - 1);
        match fin {
            b'H' | b'f' => {
                self.r = clamp_r(p(0, 1) - 1);
                self.c = clamp_c(p(1, 1) - 1);
            }
            b'd' => self.r = clamp_r(p(0, 1) - 1),
            b'G' | b'`' => self.c = clamp_c(p(0, 1) - 1),
            b'A' => self.r = self.r.saturating_sub(p(0, 1)),
            b'B' | b'e' => self.r = clamp_r(self.r + p(0, 1)),
            b'C' | b'a' => self.c = clamp_c(self.c + p(0, 1)),
            b'D' => self.c = self.c.saturating_sub(p(0, 1)),
            b'K' => {
                let row = &mut self.rows[self.r];
                let c = self.c.min(COLS);
                match n.first().copied().unwrap_or(0) {
                    1 => row[..c.min(COLS - 1) + 1].fill(' '),
                    2 => row.fill(' '),
                    _ => row[c..].fill(' '),
                }
            }
            b'J' => match n.first().copied().unwrap_or(0) {
                1 => {
                    for r in 0..self.r {
                        self.rows[r].fill(' ');
                    }
                }
                2 | 3 => {
                    for r in self.rows.iter_mut() {
                        r.fill(' ');
                    }
                }
                _ => {
                    let c = self.c.min(COLS);
                    self.rows[self.r][c..].fill(' ');
                    for r in self.r + 1..ROWS {
                        self.rows[r].fill(' ');
                    }
                }
            },
            b'X' => {
                let end = (self.c + p(0, 1)).min(COLS);
                let c = self.c.min(COLS);
                self.rows[self.r][c..end].fill(' ');
            }
            b'P' => {
                let k = p(0, 1);
                let c = self.c.min(COLS);
                let row = &mut self.rows[self.r];
                for i in c..COLS {
                    row[i] = if i + k < COLS { row[i + k] } else { ' ' };
                }
            }
            b'@' => {
                let k = p(0, 1);
                let c = self.c.min(COLS);
                let row = &mut self.rows[self.r];
                for i in (c..COLS).rev() {
                    row[i] = if i >= c + k { row[i - k] } else { ' ' };
                }
            }
            b'r' => {
                self.top = clamp_r(p(0, 1) - 1);
                self.bottom = clamp_r(p(1, ROWS) - 1);
                self.r = 0;
                self.c = 0;
            }
            b'L' => {
                for _ in 0..p(0, 1) {
                    let saved = self.top;
                    self.top = self.r;
                    self.scroll_down();
                    self.top = saved;
                }
            }
            b'M' => {
                for _ in 0..p(0, 1) {
                    let saved = self.top;
                    self.top = self.r;
                    self.scroll_up();
                    self.top = saved;
                }
            }
            _ => {}
        }
    }
}

/// Status file written by the runner: START, PHASE x, END x rc, DONE, FAIL msg.
#[derive(Debug, Default, Clone)]
pub struct Status {
    pub started: bool,
    pub phase: Option<String>,
    pub done: bool,
    pub ended: Vec<(String, i32)>,
    pub fail: Option<String>,
}

pub fn read_status(path: &Path) -> Status {
    let mut s = Status::default();
    let Ok(text) = std::fs::read_to_string(path) else { return s };
    for line in text.lines() {
        let mut it = line.splitn(3, ' ');
        match it.next() {
            Some("START") => s.started = true,
            Some("PHASE") => s.phase = it.next().map(|x| x.to_string()),
            Some("END") => {
                let ph = it.next().unwrap_or("").to_string();
                let rc = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(-1);
                s.ended.push((ph, rc));
            }
            Some("DONE") => s.done = true,
            Some("FAIL") => s.fail = Some(line.trim_start_matches("FAIL").trim().to_string()),
            _ => {}
        }
    }
    s
}

pub fn is_engine_report(name: &str) -> bool {
    name == "report.xml"
}

/// Counts PhotoRec's output (recup.1, recup.2, …) without re-reading finished folders.
#[derive(Default)]
pub struct DeepTracker {
    finished: HashMap<u32, u64>,
}

impl DeepTracker {
    /// Returns (total files, folders sorted by number).
    pub fn scan(&mut self, deep_root: &Path) -> (u64, Vec<(u32, PathBuf)>) {
        let mut dirs: Vec<(u32, PathBuf)> = std::fs::read_dir(deep_root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let n: u32 = name.strip_prefix("recup.")?.parse().ok()?;
                Some((n, e.path()))
            })
            .collect();
        dirs.sort_by_key(|d| d.0);
        let last = dirs.last().map(|d| d.0).unwrap_or(0);
        let mut total = 0;
        for (n, p) in &dirs {
            if let Some(c) = self.finished.get(n) {
                total += c;
                continue;
            }
            let c = count_files(p);
            if *n < last {
                self.finished.insert(*n, c);
            }
            total += c;
        }
        (total, dirs)
    }
}

pub fn count_files(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.file_type().map(|t| t.is_file()).unwrap_or(false)
                && !is_engine_report(&e.file_name().to_string_lossy())
        })
        .count() as u64
}

pub fn count_tree(dir: &Path) -> u64 {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_photorec_screen() {
        let t = "\x1b[5dPass 1 - Reading sector     216576/3072000, 8 files found\r\x1b[6dElapsed time 0h00m01s - Estimated time to completion 0h00m13\r";
        let p = parse_deep(t).unwrap();
        assert_eq!(p.pass, 1);
        assert_eq!(p.sector, 216576);
        assert_eq!(p.total, 3072000);
        assert_eq!(p.files, 8);
        assert_eq!(p.eta_secs, Some(13));
    }
    #[test]
    fn single_file_found() {
        let p = parse_deep("Pass 2 - Reading sector 10/20, 1 file found").unwrap();
        assert_eq!(p.files, 1);
        assert_eq!(p.pass, 2);
    }

    #[test]
    fn replays_ncurses_partial_updates() {
        let raw = b"\x1b[H\x1b[2J\x1b[11dPass 1 - Reading sector    1316168/6144000, 14 files found\r\x1b[12dElapsed time 0h00m01s - Estimated time to completion 0h00m03\r\x1b[19;22r\x1b[19;1H\x1bM\x1b[1;24r\x1b[11;28H3139544\x1b[11;46H5\x1b[12;20H2\x1b[12;60H1\r";
        let mut s = Screen::default();
        s.feed(&raw[..40]);
        s.feed(&raw[40..]);
        let p = parse_deep(&s.text()).unwrap();
        assert_eq!(p.sector, 3139544);
        assert_eq!(p.total, 6144000);
        assert_eq!(p.files, 15);
        assert_eq!(p.eta_secs, Some(1));
    }
}
