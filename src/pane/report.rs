use std::collections::{HashMap, HashSet};

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

    let orders: HashMap<&str, i64> = data
      .lines()
      .filter_map(|line| {
        let rest = line.strip_prefix("uda.taskwarrior-tui.report.")?;
        let (key, value) = rest.split_once(char::is_whitespace)?;
        let name = key.strip_suffix(".order")?;
        Some((name, value.trim().parse().ok()?))
      })
      .collect();
    report_names.sort_by(|a, b| match (orders.get(a.as_str()), orders.get(b.as_str())) {
      (Some(a_order), Some(b_order)) => a_order.cmp(b_order).then_with(|| a.cmp(b)),
      (Some(_), None) => std::cmp::Ordering::Less,
      (None, Some(_)) => std::cmp::Ordering::Greater,
      (None, None) => a.cmp(b),
    });

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

  #[test]
  fn ordered_reports_precede_unordered_and_search_preserves_order() {
    let data = "report.zulu.columns id\nreport.alpha.columns id\nreport.beta.columns id\nreport.gamma.columns id\nreport.gamma.filter +work\nuda.taskwarrior-tui.report.zulu.order -1\nuda.taskwarrior-tui.report.beta.order 2\nuda.taskwarrior-tui.report.missing.order -99\n";
    let mut reports = ReportsState::new();
    reports.update_data("alpha", data);
    assert_eq!(
      reports.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
      vec!["zulu", "beta", "alpha", "gamma"]
    );
    reports.search = "a".into();
    assert_eq!(reports.filtered_indices(), vec![1, 2, 3]);
    reports.search = "+WORK".into();
    assert_eq!(reports.filtered_indices(), vec![3]);
  }

  #[test]
  fn ties_and_invalid_or_absent_orders_are_alphabetical() {
    let data = "report.zulu.columns id\nreport.beta.columns id\nreport.alpha.columns id\nreport.gamma.columns id\nuda.taskwarrior-tui.report.zulu.order 4\nuda.taskwarrior-tui.report.beta.order 4\nuda.taskwarrior-tui.report.alpha.order 1.5\nuda.taskwarrior-tui.report.gamma.order nope\n";
    let mut reports = ReportsState::new();
    reports.update_data("", data);
    assert_eq!(
      reports.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
      vec!["beta", "zulu", "alpha", "gamma"]
    );
    reports.update_data("", "report.zulu.columns id\nreport.alpha.columns id\n");
    assert_eq!(reports.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["alpha", "zulu"]);
  }
}
