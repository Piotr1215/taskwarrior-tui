use std::collections::HashSet;

use crate::{config::Config, table::TaskwarriorTuiTableState};

const NAME: &str = "Name";
const FILTER: &str = "Filter";
const ACTIVE: &str = "Active";

#[derive(Debug, Clone, Default)]
pub struct ReportDetails {
  pub name: String,
  pub filter: String,
  pub active: String,
}

impl ReportDetails {
  pub fn new(name: String, filter: String, active: String) -> Self {
    Self { name, filter, active }
  }
}

pub struct ReportsState {
  pub table_state: TaskwarriorTuiTableState,
  pub columns: Vec<String>,
  pub rows: Vec<ReportDetails>,
  /// Current search query typed by the user inside the popup.
  pub search: String,
}

impl ReportsState {
  pub(crate) fn new() -> Self {
    Self {
      table_state: TaskwarriorTuiTableState::default(),
      columns: vec![NAME.to_string(), FILTER.to_string(), ACTIVE.to_string()],
      rows: vec![],
      search: String::new(),
    }
  }

  pub fn len(&self) -> usize {
    self.rows.len()
  }

  pub fn is_empty(&self) -> bool {
    self.rows.is_empty()
  }

  /// Returns the indices into `self.rows` that match the current search query.
  /// An empty query matches everything (original order is preserved).
  /// Matching is case-insensitive substring on name or filter.
  pub fn filtered_indices(&self) -> Vec<usize> {
    let query = self.search.to_lowercase();
    self
      .rows
      .iter()
      .enumerate()
      .filter(|(_, r)| query.is_empty() || r.name.to_lowercase().contains(&query) || r.filter.to_lowercase().contains(&query))
      .map(|(i, _)| i)
      .collect()
  }

  /// Parse the `task show` output for all reports that have columns defined
  /// (i.e. reports that produce tabular task output). Collects their name,
  /// effective filter, and whether they are the currently active report.
  pub fn update_data(&mut self, current_report: &str, data: &str) {
    self.rows.clear();

    // Collect unique report names that have a `.columns` entry.
    let mut seen: HashSet<String> = HashSet::new();
    let mut report_names: Vec<String> = vec![];

    for line in data.split('\n') {
      if let Some(rest) = line.strip_prefix("report.") {
        // Key has format: "report.<name>.<property>  <value>"
        // We split off the first whitespace to isolate the full key.
        let (key, _) = match rest.split_once(char::is_whitespace) {
          Some(pair) => pair,
          None => continue,
        };

        if let Some(name) = key.strip_suffix(".columns")
          && seen.insert(name.to_string())
        {
          report_names.push(name.to_string());
        }
      }
    }

    report_names.sort();

    for name in report_names {
      let filter = Config::get_filter(data, &name).unwrap_or_default();
      let active = if name == current_report { "yes" } else { "no" };
      self.rows.push(ReportDetails::new(name, filter, active.to_string()));
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn report_rows_use_effective_filter_and_search_it() {
    let data = "report.alpha.columns id,description\nreport.alpha.filter +work\nreport.beta.columns id\nreport.beta.filter +old\nuda.taskwarrior-tui.task-report.beta.filter +new\n";
    let mut reports = ReportsState::new();
    reports.update_data("beta", data);
    assert_eq!(reports.columns, vec!["Name", "Filter", "Active"]);
    assert_eq!(reports.rows[0].filter, "+work");
    assert_eq!(reports.rows[0].active, "no");
    assert_eq!(reports.rows[1].filter, "+new");
    assert_eq!(reports.rows[1].active, "yes");
    reports.search = "NEW".into();
    assert_eq!(reports.filtered_indices(), vec![1]);
    reports.search = "ALP".into();
    assert_eq!(reports.filtered_indices(), vec![0]);
  }
}
