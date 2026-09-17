use crate::flow::{self, Flow};
use anyhow::Result;
use std::path::PathBuf;

pub enum Mode {
    Normal,
    Filter,
    Detail,
    Help,
}

pub struct App {
    pub flows: Vec<Flow>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter_text: String,
    pub mode: Mode,
    pub status: Option<String>,
    pub should_quit: bool,
    /// When true (the default), a reload jumps the selection to the
    /// newest flow — "follow" mode. Toggling this off with `f` lets you
    /// inspect an older flow without the list yanking your selection
    /// out from under you the next time new traffic arrives.
    pub follow_live: bool,
    log_path: PathBuf,
}

impl App {
    pub fn new(log_path: PathBuf) -> Result<Self> {
        let mut app = Self {
            flows: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            filter_text: String::new(),
            mode: Mode::Normal,
            status: None,
            should_quit: false,
            follow_live: true,
            log_path,
        };
        app.reload();
        Ok(app)
    }

    /// Re-reads the capture log from disk. In follow mode this jumps to
    /// the newest flow (index 0, since `apply_sort_and_filter` sorts
    /// newest-first); otherwise it tries to keep the same flow selected
    /// by identity (pipeline + timestamp) so browsing isn't disrupted by
    /// new traffic landing in the background.
    pub fn reload(&mut self) {
        let previously_selected = self.selected_flow().map(|f| (f.pipeline.clone(), f.at));
        self.flows = flow::read_all(&self.log_path).unwrap_or_default();
        self.apply_sort_and_filter();

        if self.follow_live {
            self.selected = 0;
        } else if let Some((pipeline, at)) = previously_selected
            && let Some(pos) = self
                .filtered
                .iter()
                .position(|&i| self.flows[i].pipeline == pipeline && self.flows[i].at == at)
        {
            self.selected = pos;
        }
    }

    /// Newest first — the thing you just want to glance at belongs at the
    /// top, same rationale as every other live-monitoring TUI this
    /// session (Argus, cyberwatch).
    pub fn apply_sort_and_filter(&mut self) {
        self.flows.sort_by_key(|f| std::cmp::Reverse(f.at));
        let needle = self.filter_text.to_lowercase();
        self.filtered = self
            .flows
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                needle.is_empty()
                    || f.pipeline.to_lowercase().contains(&needle)
                    || f.rendered.to_lowercase().contains(&needle)
            })
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn selected_flow(&self) -> Option<&Flow> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.flows.get(i))
    }

    /// Exposed so `main.rs`'s event loop can poll the capture log's mtime
    /// independently of a full `reload()` — checking "did anything change"
    /// should be cheap and not itself re-parse the whole file every tick.
    pub fn log_path_for_polling(&self) -> &std::path::Path {
        &self.log_path
    }

    pub fn next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn toggle_follow(&mut self) {
        self.follow_live = !self.follow_live;
        self.status = Some(if self.follow_live {
            "following live traffic".to_string()
        } else {
            "browsing — new traffic won't move your selection".to_string()
        });
    }
}

pub fn detail_lines(flow: &Flow) -> String {
    format!(
        "pipeline: {}\ndirection: {}\nat:        {}\nformat:    {}\n\n{}",
        flow.pipeline,
        flow.direction,
        flow.at.to_rfc3339(),
        flow.format,
        flow.rendered
    )
}
