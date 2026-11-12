use std::{
  cmp::Ordering,
  collections::{HashMap, HashSet},
  sync::LazyLock,
};

use regex::Regex;
use task_hookrs::{status::TaskStatus, task::Task, uda::UDAValue};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
  #[default]
  Execution,
  Due,
  Priority,
  Newest,
  Oldest,
  Title,
}

impl Sort {
  pub fn parse(value: &str) -> Option<Self> {
    match value.trim().to_lowercase().as_str() {
      "next" => Some(Self::Execution),
      "due" => Some(Self::Due),
      "prio" | "priority" => Some(Self::Priority),
      "newest" => Some(Self::Newest),
      "oldest" => Some(Self::Oldest),
      "title" => Some(Self::Title),
      _ => None,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewOptions {
  pub sort: Sort,
  pub ready_only: bool,
  pub handoffs_only: bool,
  // Linear workflow states whose rows this view hides, compared without case.
  pub hide_statuses: Vec<String>,
  // Drop a project header when none of its rows survive the other filters.
  pub hide_empty: bool,
  // Drop projects that exist only in Taskwarrior (view_key local:...).
  pub hide_local: bool,
}

// A named set of Projects view settings, the way a Taskwarrior report names a
// filter and sort. Reports change only what the view shows, never task data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
  pub name: String,
  pub options: ViewOptions,
  pub collapsed: bool,
}

impl Report {
  fn new(name: &str, sort: Sort) -> Self {
    Self {
      name: name.into(),
      options: ViewOptions { sort, ..Default::default() },
      collapsed: false,
    }
  }
}

pub const REPORT_PREFIX: &str = "uda.taskwarrior-tui.projects.report.";
pub const REPORT_LIST: &str = "uda.taskwarrior-tui.projects.reports";

// The former sort keys and toggles, now reports. `next` stays the default.
pub fn builtin_reports() -> Vec<Report> {
  let with = |name: &str, change: fn(&mut Report)| {
    let mut report = Report::new(name, Sort::Execution);
    change(&mut report);
    report
  };
  vec![
    Report::new("next", Sort::Execution),
    with("todo", |r| {
      r.options.hide_statuses = ["Triage", "Backlog", "Parked"].map(String::from).to_vec();
    }),
    with("active", |r| r.options.hide_empty = true),
    with("handoffs", |r| {
      r.options.handoffs_only = true;
      r.options.hide_empty = true;
    }),
    with("ready", |r| r.options.ready_only = true),
    Report::new("due", Sort::Due),
    Report::new("prio", Sort::Priority),
    Report::new("newest", Sort::Newest),
    Report::new("oldest", Sort::Oldest),
  ]
}

// Read the reports from `task show` style settings. `get` returns a setting's
// value; `defined` lists every report name that has at least one setting. The
// optional list fixes the cycle order; without it, the built-ins come first
// and other defined reports follow in the order they appear.
pub fn reports(get: impl Fn(&str) -> Option<String>, defined: &[String]) -> Vec<Report> {
  let builtins = builtin_reports();
  let names: Vec<String> = match get(REPORT_LIST) {
    Some(list) if list.split(',').any(|name| !name.trim().is_empty()) => list.split(',').map(str::trim).map(str::to_owned).collect(),
    _ => builtins.iter().map(|report| report.name.clone()).chain(defined.iter().cloned()).collect(),
  };
  let mut seen = HashSet::new();
  let flag = |name: &str, setting: &str| {
    get(&format!("{REPORT_PREFIX}{name}.{setting}")).map(|value| crate::config::TaskWarriorBool::get_bool(value.as_str()))
  };
  names
    .into_iter()
    .filter(|name| !name.is_empty() && !name.contains(char::is_whitespace) && seen.insert(name.clone()))
    .map(|name| {
      let mut report = builtins
        .iter()
        .find(|report| report.name == name)
        .cloned()
        .unwrap_or_else(|| Report::new(&name, Sort::Execution));
      if let Some(sort) = get(&format!("{REPORT_PREFIX}{name}.sort")).as_deref().and_then(Sort::parse) {
        report.options.sort = sort;
      }
      if let Some(statuses) = get(&format!("{REPORT_PREFIX}{name}.hide-status")) {
        report.options.hide_statuses = statuses
          .split(',')
          .map(str::trim)
          .filter(|status| !status.is_empty())
          .map(str::to_owned)
          .collect();
      }
      let options = &mut report.options;
      for (setting, field) in [
        ("ready-only", &mut options.ready_only),
        ("handoffs-only", &mut options.handoffs_only),
        ("hide-empty", &mut options.hide_empty),
        ("hide-local", &mut options.hide_local),
      ] {
        if let Some(Some(value)) = flag(&name, setting) {
          *field = value;
        }
      }
      if let Some(Some(value)) = flag(&name, "collapsed") {
        report.collapsed = value;
      }
      report
    })
    .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowPresentation {
  pub project: String,
  pub kind: String,
  pub state: String,
  pub action: String,
  pub description: String,
  pub linear: String,
  pub linear_status: String,
}

pub fn has_tag(task: &Task, tag: &str) -> bool {
  task.tags().as_ref().is_some_and(|tags| tags.iter().any(|t| t == tag))
}

// Explicit project rows are headers; legacy +view rows keep their old rules.
pub fn is_overview(task: &Task) -> bool {
  matches!(task.uda().get("view_kind"), Some(UDAValue::Str(kind)) if kind == "project")
    || has_tag(task, "view")
      && match task.uda().get("view_key") {
        Some(UDAValue::Str(key)) if !key.is_empty() => key.starts_with("standing:"),
        None | Some(UDAValue::Str(_)) => true,
        _ => false,
      }
}

// Projects created for us live only in Taskwarrior; Linear headers use standing:.
fn is_local(task: &Task) -> bool {
  is_overview(task) && matches!(task.uda().get("view_key"), Some(UDAValue::Str(key)) if key.starts_with("local:"))
}

// Use the configured shortcuts rather than starting an issue merely to open
// its spawn dialog or source. The jump helper owns the live-session check.
pub fn start_shortcut(task: &Task) -> Option<usize> {
  if is_overview(task) || has_tag(task, "piotr") {
    return None;
  }
  if has_tag(task, "view") && matches!(task.uda().get("view_key"), Some(UDAValue::Str(key)) if key.starts_with("issue:") || key.starts_with("ask:")) {
    return Some(1);
  }
  if matches!(task.uda().get("linear_issue_id"), Some(UDAValue::Str(id)) if !id.trim().is_empty()) {
    static SESSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tmux session [A-Za-z0-9._-]+").unwrap());
    let recorded = task
      .annotations()
      .into_iter()
      .flatten()
      .any(|annotation| SESSION.is_match(annotation.description()));
    return Some(if recorded && (task.start().is_some() || has_tag(task, "ACTIVE")) {
      0
    } else {
      4
    });
  }
  None
}

// What `s` does on a row, named the way the boards name it. The column and the
// key share start_shortcut(), so they cannot disagree. Headers have no action.
pub fn action(task: &Task) -> String {
  if is_overview(task) {
    return "brief".into();
  }
  let status = |name: &str| linear_state(task).is_some_and(|status| status.trim().eq_ignore_ascii_case(name));
  let name = match start_shortcut(task) {
    Some(1) if uda_string(task, "view_key").is_some_and(|key| key.starts_with("ask:")) => "review",
    Some(1) if status("triage") => "triage",
    Some(1) => "open",
    Some(0) => "jump",
    Some(_) => "spawn",
    None if task.start().is_some() => "stop",
    // Starting a handoff runs its kind through the on-modify navigator.
    None if has_tag(task, "piotr") => uda_string(task, "kind").unwrap_or("start"),
    None => "start",
  };
  name.to_string()
}

// Administrative work is global, even when a standing admin row exists.
fn project(task: &Task) -> Option<&str> {
  task.project().map(String::as_str).filter(|p| !p.is_empty() && *p != "admin")
}

fn membership(task: &Task) -> Option<&str> {
  match task.uda().get("view_project") {
    Some(UDAValue::Str(project)) if project == "!unassigned" => None,
    Some(UDAValue::Str(project)) if !project.is_empty() => Some(project),
    _ => project(task),
  }
}

fn headers(tasks: &[Task]) -> HashSet<&str> {
  tasks.iter().filter(|t| is_overview(t)).filter_map(project).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectGroup {
  Named(String),
  Unsorted,
}

pub fn project_group(task: &Task, context: &[Task]) -> ProjectGroup {
  if is_overview(task) {
    return project(task).map_or(ProjectGroup::Unsorted, |name| ProjectGroup::Named(name.to_owned()));
  }
  let projects = headers(context);
  membership(task)
    .filter(|name| projects.contains(name))
    .map_or(ProjectGroup::Unsorted, |name| ProjectGroup::Named(name.to_owned()))
}

// Steps whose rank moved past a step that was ahead of them in the same
// project, between two exports. Only the moved step counts: a renumber keeps
// the order, and a step that falls back leaves the others' ranks alone.
pub fn jumped(before: &[Task], after: &[Task]) -> Vec<Uuid> {
  let ranked = |tasks: &[Task]| -> Vec<(Uuid, ProjectGroup, f64)> {
    tasks
      .iter()
      .filter(|task| !is_overview(task))
      .filter_map(|task| match project_group(task, tasks) {
        ProjectGroup::Named(name) => Some((*task.uuid(), ProjectGroup::Named(name), view_rank(task)?)),
        ProjectGroup::Unsorted => None,
      })
      .collect()
  };
  let old: HashMap<_, _> = ranked(before).into_iter().map(|(uuid, group, rank)| (uuid, (group, rank))).collect();
  let both: Vec<_> = ranked(after)
    .into_iter()
    .filter_map(|(uuid, group, now)| {
      let (was_group, was) = old.get(&uuid)?;
      (*was_group == group).then_some((uuid, group, *was, now))
    })
    .collect();
  both
    .iter()
    .filter(|(_, group, was, now)| {
      now < was
        && both
          .iter()
          .any(|(_, other_group, other_was, other_now)| other_group == group && other_was < was && other_now > now)
    })
    .map(|(uuid, ..)| *uuid)
    .collect()
}

fn finished(task: &Task) -> bool {
  matches!(task.status(), TaskStatus::Completed | TaskStatus::Deleted)
}

fn fallback_order(a: &Task, b: &Task) -> Ordering {
  let rank = |task: &Task| match task.priority().map(String::as_str) {
    Some("H") => 0,
    Some("M") => 1,
    Some("L") => 2,
    _ => 3,
  };
  rank(a)
    .cmp(&rank(b))
    .then_with(|| match (a.due(), b.due()) {
      (Some(a), Some(b)) => (**a).cmp(&**b),
      (Some(_), None) => Ordering::Less,
      (None, Some(_)) => Ordering::Greater,
      _ => Ordering::Equal,
    })
    .then_with(|| (**b.entry()).cmp(&**a.entry()))
    .then_with(|| a.uuid().cmp(b.uuid()))
}

// Kahn's algorithm chooses among available actions using the existing priority
// and newest-first order, with due dates breaking equal-priority ties. Cycles
// remain visible in that same deterministic order instead of losing rows.
fn execution_order(tasks: Vec<Task>, unready: &HashSet<Uuid>) -> Vec<Task> {
  dependency_order(tasks, |a, b| {
    unready
      .contains(a.uuid())
      .cmp(&unready.contains(b.uuid()))
      .then_with(|| fallback_order(a, b))
  })
}

fn dependency_order(mut tasks: Vec<Task>, compare: impl FnMut(&Task, &Task) -> Ordering) -> Vec<Task> {
  tasks.sort_by(compare);
  let indices: HashMap<_, _> = tasks.iter().enumerate().map(|(i, t)| (*t.uuid(), i)).collect();
  let mut remaining = vec![0; tasks.len()];
  let mut dependents = vec![Vec::new(); tasks.len()];
  for (i, task) in tasks.iter().enumerate() {
    for dependency in task.depends().into_iter().flatten().collect::<HashSet<_>>() {
      if let Some(&prerequisite) = indices.get(dependency)
        && !finished(&tasks[prerequisite])
      {
        remaining[i] += 1;
        dependents[prerequisite].push(i);
      }
    }
  }
  let mut pending: Vec<_> = tasks.into_iter().map(Some).collect();
  let mut result = Vec::with_capacity(pending.len());
  while let Some(i) = pending.iter().enumerate().position(|(i, t)| t.is_some() && remaining[i] == 0) {
    result.push(pending[i].take().unwrap());
    for &dependent in &dependents[i] {
      remaining[dependent] -= 1;
    }
  }
  result.extend(pending.into_iter().flatten());
  result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Readiness {
  Ready,
  Running,
  Waiting,
  Blocked,
  Cycle,
  Missing,
  Finished,
}

impl Readiness {
  fn ready(self) -> bool {
    matches!(self, Self::Ready | Self::Running)
  }

  fn label(self) -> &'static str {
    match self {
      Self::Ready => "ready",
      Self::Running => "running",
      Self::Waiting => "waiting",
      Self::Blocked => "blocked",
      Self::Cycle => "cycle",
      Self::Missing => "missing dep",
      Self::Finished => "done",
    }
  }
}

fn readiness(task: &Task, context: &HashMap<Uuid, &Task>) -> Readiness {
  if finished(task) {
    return Readiness::Finished;
  }
  if dependency_cycle(*task.uuid(), context) {
    return Readiness::Cycle;
  }
  if task.depends().into_iter().flatten().any(|dep| !context.contains_key(dep)) {
    return Readiness::Missing;
  }
  if task
    .depends()
    .into_iter()
    .flatten()
    .any(|dep| context.get(dep).is_some_and(|dep| !finished(dep)))
  {
    return Readiness::Blocked;
  }
  if has_tag(task, "BLOCKED") {
    return Readiness::Blocked;
  }
  if task.status() == &TaskStatus::Waiting || task.wait().is_some_and(|date| **date > chrono::Utc::now().naive_utc()) {
    return Readiness::Waiting;
  }
  if task.start().is_some() || has_tag(task, "ACTIVE") {
    return Readiness::Running;
  }
  Readiness::Ready
}

fn uda_string<'a>(task: &'a Task, name: &str) -> Option<&'a str> {
  match task.uda().get(name) {
    Some(UDAValue::Str(value)) if !value.is_empty() => Some(value),
    _ => None,
  }
}

// A task's Linear workflow state. Mirror rows from the producer carry it in
// view_state; tasks synced from Linear carry it in linear_status.
fn linear_state(task: &Task) -> Option<&str> {
  uda_string(task, "linear_status").or_else(|| (generated(task) && !is_overview(task)).then(|| uda_string(task, "view_state")).flatten())
}

fn generated(task: &Task) -> bool {
  has_tag(task, "view")
    && !has_tag(task, "piotr")
    && (is_overview(task) || uda_string(task, "view_key").is_some_and(|key| key.starts_with("issue:") || key.starts_with("ask:")))
}

// The producer keeps legacy descriptions for compatibility. Recognize only
// its precise no-task shape, and never parse ordinary handoff prose.
fn legacy_source(task: &Task) -> Option<(&str, &str, &str)> {
  if !generated(task) || is_overview(task) {
    return None;
  }
  static SOURCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^no task · ([A-Z][A-Z0-9]*-[0-9]+) ([^·\r\n]+) · (.+)$").unwrap());
  let captures = SOURCE.captures(task.description())?;
  Some((captures.get(1)?.as_str(), captures.get(2)?.as_str().trim(), captures.get(3)?.as_str()))
}

fn legacy_header(task: &Task) -> Option<(&str, &str, &str)> {
  if !generated(task) || !is_overview(task) {
    return None;
  }
  static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^standing · ([A-Z][A-Z0-9]*-[0-9]+) (.+) \(([^()\r\n]+)\)$").unwrap());
  let description = task.description().strip_suffix(" · source unavailable").unwrap_or(task.description());
  let captures = HEADER.captures(description)?;
  let state = captures.get(3)?.as_str().trim();
  if state.is_empty() {
    return None;
  }
  Some((captures.get(1)?.as_str(), state, captures.get(2)?.as_str()))
}

fn clean_description(task: &Task) -> &str {
  if !generated(task) {
    return task.description();
  }
  if let Some(title) = uda_string(task, "view_title") {
    return title;
  }
  if is_overview(task) {
    if let Some((_, _, title)) = legacy_header(task) {
      return title;
    }
    let title = task.description().strip_prefix("standing · ").unwrap_or(task.description());
    return title.strip_suffix(" · source unavailable").unwrap_or(title);
  }
  legacy_source(task).map_or(task.description(), |(_, _, title)| title)
}

fn due_order(a: &Task, b: &Task) -> Ordering {
  match (a.due(), b.due()) {
    (Some(a), Some(b)) => (**a).cmp(&**b),
    (Some(_), None) => Ordering::Less,
    (None, Some(_)) => Ordering::Greater,
    _ => Ordering::Equal,
  }
}

fn view_rank(task: &Task) -> Option<f64> {
  let value = match task.uda().get("view_rank")? {
    UDAValue::U64(value) => *value as f64,
    UDAValue::F64(value) => *value,
    UDAValue::Str(value) => value.parse().ok()?,
  };
  value.is_finite().then_some(value)
}

fn view_order(a: &Task, b: &Task, sort: Sort, states: &HashMap<Uuid, Readiness>) -> Ordering {
  let order = match sort {
    Sort::Execution => match (view_rank(a), view_rank(b)) {
      (Some(a), Some(b)) => a.total_cmp(&b),
      (Some(_), None) => Ordering::Less,
      (None, Some(_)) => Ordering::Greater,
      (None, None) => (!states[a.uuid()].ready()).cmp(&!states[b.uuid()].ready()),
    },
    Sort::Due => due_order(a, b),
    Sort::Priority => Ordering::Equal,
    Sort::Newest => (**b.entry()).cmp(&**a.entry()),
    Sort::Oldest => (**a.entry()).cmp(&**b.entry()),
    Sort::Title => clean_description(a).to_lowercase().cmp(&clean_description(b).to_lowercase()),
  };
  order.then_with(|| fallback_order(a, b))
}

// The Projects tab owns presentation order and filters. Sorting the full input
// before hiding rows preserves transitive prerequisites through hidden tasks.
// The legacy Tasks report continues to use group() and descriptions().
pub fn arrange(tasks: Vec<Task>, options: &ViewOptions) -> Vec<Task> {
  let by_uuid: HashMap<_, _> = tasks.iter().map(|task| (*task.uuid(), task)).collect();
  let states: HashMap<_, _> = tasks.iter().map(|task| (*task.uuid(), readiness(task, &by_uuid))).collect();
  let projects: HashSet<_> = headers(&tasks).into_iter().map(str::to_owned).collect();
  let mut overviews = Vec::new();
  let mut children: HashMap<String, Vec<Task>> = HashMap::new();
  let mut unmatched = Vec::new();
  let mut seen = HashSet::new();
  for task in tasks {
    if !seen.insert(*task.uuid()) {
      continue;
    }
    if is_overview(&task) {
      overviews.push(task);
    } else if let Some(project) = membership(&task).filter(|project| projects.contains(*project)) {
      children.entry(project.to_string()).or_default().push(task);
    } else {
      unmatched.push(task);
    }
  }
  overviews.sort_by(|a, b| {
    let a_project = a.project().map(String::as_str).unwrap_or_default();
    let b_project = b.project().map(String::as_str).unwrap_or_default();
    a_project
      .is_empty()
      .cmp(&b_project.is_empty())
      .then_with(|| a_project.to_lowercase().cmp(&b_project.to_lowercase()))
      .then_with(|| a.uuid().cmp(b.uuid()))
  });
  let hidden_status =
    |task: &Task| linear_state(task).is_some_and(|status| options.hide_statuses.iter().any(|hide| hide.eq_ignore_ascii_case(status.trim())));
  let keep =
    |task: &Task| (!options.ready_only || states[task.uuid()].ready()) && (!options.handoffs_only || has_tag(task, "piotr")) && !hidden_status(task);
  let mut result = Vec::new();
  for header in overviews {
    let project = project(&header).map(str::to_owned);
    let rows: Vec<Task> = project
      .and_then(|project| children.remove(&project))
      .map(|group| {
        dependency_order(group, |a, b| view_order(a, b, options.sort, &states))
          .into_iter()
          .filter(&keep)
          .collect()
      })
      .unwrap_or_default();
    // A hidden project takes its rows with it rather than sending them to Unsorted.
    if (options.hide_local && is_local(&header)) || (options.hide_empty && rows.is_empty()) {
      continue;
    }
    result.push(header);
    result.extend(rows);
  }
  result.extend(
    dependency_order(unmatched, |a, b| view_order(a, b, options.sort, &states))
      .into_iter()
      .filter(keep),
  );
  result
}

pub fn presentation(tasks: &[Task], context: &[Task]) -> Vec<RowPresentation> {
  let mut projects = headers(context);
  projects.extend(headers(tasks));
  let by_uuid: HashMap<_, _> = context.iter().chain(tasks).map(|task| (*task.uuid(), task)).collect();
  tasks
    .iter()
    .map(|task| {
      let overview = is_overview(task);
      let generated = generated(task);
      let grouped = !overview && membership(task).is_some_and(|project| projects.contains(project));
      let inferred_kind = if overview {
        "project"
      } else if has_tag(task, "piotr") {
        "handoff"
      } else if generated && uda_string(task, "view_key").is_some_and(|key| key.starts_with("issue:") || key.starts_with("ask:"))
        || uda_string(task, "linear_issue_id").is_some()
      {
        "issue"
      } else {
        "task"
      };
      let kind = generated
        .then(|| uda_string(task, "view_kind"))
        .flatten()
        .and_then(|kind| match kind {
          "project" | "standing" => Some("project"),
          "handoff" => Some("handoff"),
          "issue" | "ask" | "review" => Some("issue"),
          "task" => Some("task"),
          _ => None,
        })
        .unwrap_or(inferred_kind);
      let source = if overview { legacy_header(task) } else { legacy_source(task) };
      let state = if overview && generated {
        uda_string(task, "view_state")
          .or_else(|| (task.description().ends_with(" · source unavailable")).then_some("source unavailable"))
          .or_else(|| source.map(|(_, state, _)| state))
          .unwrap_or_default()
          .to_string()
      } else {
        readiness(task, &by_uuid).label().to_string()
      };
      let source_issue = generated
        .then(|| {
          uda_string(task, "view_issue")
            .or_else(|| source.map(|(issue, _, _)| issue))
            .or_else(|| uda_string(task, "view_key").and_then(|key| key.strip_prefix("issue:")))
        })
        .flatten();
      let linear_id = source_issue.or_else(|| uda_string(task, "linear_issue_id")).unwrap_or_default();
      let linear_status = linear_state(task).unwrap_or_default().to_string();
      RowPresentation {
        project: if overview {
          task.project().cloned().unwrap_or_default()
        } else if grouped {
          String::new()
        } else {
          "Unsorted".into()
        },
        kind: kind.into(),
        state,
        action: action(task),
        description: format!("{}{}", if grouped { "  ↳ " } else { "" }, clean_description(task)),
        linear: linear_id.to_string(),
        linear_status,
      }
    })
    .collect()
}

// Containment uses exact project identity, never depends or a prose match.
// Every input task UUID appears once. A duplicate header remains a row, but does
// not duplicate children. Orphans retain their original top-level position.
pub fn group(tasks: Vec<Task>) -> Vec<Task> {
  let projects: HashSet<_> = headers(&tasks).into_iter().map(str::to_owned).collect();
  let by_uuid: HashMap<_, _> = tasks.iter().map(|t| (*t.uuid(), t)).collect();
  let unready = tasks
    .iter()
    .filter(|t| {
      t.status() == &TaskStatus::Waiting
        || t.wait().is_some_and(|date| **date > chrono::Utc::now().naive_utc())
        || t
          .depends()
          .into_iter()
          .flatten()
          .any(|dep| by_uuid.get(dep).is_none_or(|task| !finished(task)))
    })
    .map(|t| *t.uuid())
    .collect();
  let mut children: HashMap<String, Vec<Task>> = HashMap::new();
  let mut rows = Vec::new();
  let mut seen = HashSet::new();
  for task in tasks {
    if !seen.insert(*task.uuid()) {
      continue;
    }
    if !is_overview(&task)
      && let Some(project) = membership(&task).filter(|p| projects.contains(*p))
    {
      children.entry(project.to_string()).or_default().push(task);
    } else {
      rows.push(task);
    }
  }
  let mut result = Vec::new();
  for row in rows {
    let project = is_overview(&row).then(|| project(&row)).flatten().map(str::to_owned);
    result.push(row);
    if let Some(group) = project.and_then(|p| children.remove(&p)) {
      result.extend(execution_order(group, &unready));
    }
  }
  result
}

fn dependency_cycle(uuid: Uuid, tasks: &HashMap<Uuid, &Task>) -> bool {
  let mut visited = HashSet::new();
  let mut next = tasks.get(&uuid).and_then(|t| t.depends()).cloned().unwrap_or_default();
  while let Some(dependency) = next.pop() {
    if dependency == uuid {
      return true;
    }
    if visited.insert(dependency)
      && let Some(task) = tasks.get(&dependency).filter(|t| !finished(t))
    {
      next.extend(task.depends().into_iter().flatten());
    }
  }
  false
}

pub fn descriptions(tasks: &[Task]) -> Vec<String> {
  let projects = headers(tasks);
  let by_uuid: HashMap<_, _> = tasks.iter().map(|t| (*t.uuid(), t)).collect();
  tasks
    .iter()
    .map(|task| {
      if is_overview(task) {
        return String::new();
      }
      let location = if matches!(task.uda().get("view_project"), Some(UDAValue::Str(project)) if project == "!unassigned") {
        "[unresolved project] ".to_string()
      } else if membership(task).is_some_and(|p| projects.contains(p)) {
        "  ↳ ".to_string()
      } else {
        format!(
          "[ungrouped: {}] ",
          membership(task)
            .or_else(|| task.project().map(String::as_str))
            .filter(|p| !p.is_empty())
            .unwrap_or("no project")
        )
      };
      let mut blocked = Vec::new();
      let mut absent = Vec::new();
      for dependency in task.depends().into_iter().flatten() {
        match by_uuid.get(dependency) {
          Some(t) if finished(t) => (),
          Some(t) => blocked.push(t.id().map_or_else(|| dependency.to_string(), |id| id.to_string())),
          None => absent.push(dependency.to_string()),
        }
      }
      let mut state = Vec::new();
      if task.status() == &TaskStatus::Waiting || task.wait().is_some_and(|date| **date > chrono::Utc::now().naive_utc()) {
        state.push("waiting".to_string());
      }
      if dependency_cycle(*task.uuid(), &by_uuid) {
        state.push("dependency cycle".to_string());
      } else if !blocked.is_empty() {
        state.push(format!("blocked: {}", blocked.join(",")));
      }
      if !absent.is_empty() {
        state.push(format!("dependency not in report: {}", absent.join(",")));
      }
      if state.is_empty() {
        state.push("ready".to_string());
      }
      format!("{location}[{}] ", state.join("; "))
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;
  use task_hookrs::{date::Date, task::TaskBuilder};

  use super::*;

  fn task(id: u128, project: Option<&str>, tags: &[&str]) -> Task {
    let mut task = TaskBuilder::default()
      .uuid(Uuid::from_u128(id))
      .id(Some(id as u64))
      .description(format!("task {id}"))
      .entry(Date::from(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()))
      .build()
      .unwrap();
    task.set_project(project);
    task.set_tags(Some(tags.iter().map(|tag| tag.to_string()).collect::<Vec<_>>()));
    task
  }

  fn ids(tasks: &[Task]) -> Vec<u128> {
    tasks.iter().map(|task| task.uuid().as_u128()).collect()
  }

  fn depends(task: &mut Task, ids: &[u128]) {
    task.set_depends::<_, Uuid>(Some(ids.iter().map(|id| Uuid::from_u128(*id))));
  }

  fn view(task: &mut Task, key: &str, value: &str) {
    task.uda_mut().insert(key.into(), UDAValue::Str(value.into()));
  }

  fn date(year: i32) -> Date {
    Date::from(NaiveDate::from_ymd_opt(year, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap())
  }

  const SORTS: [Sort; 6] = [Sort::Execution, Sort::Due, Sort::Priority, Sort::Newest, Sort::Oldest, Sort::Title];

  #[test]
  fn next_queue_orders_ranks_and_prerequisites_and_preserves_rows() {
    let mut issue = task(2, Some("ci"), &["view"]);
    view(&mut issue, "view_key", "issue:ABC-1");
    view(&mut issue, "view_state", "Triage");
    issue.uda_mut().insert("view_rank".into(), UDAValue::U64(10));
    depends(&mut issue, &[4]);
    let mut handoff = task(3, Some("ci"), &["piotr"]);
    handoff.uda_mut().insert("view_rank".into(), UDAValue::F64(20.5));
    let mut prerequisite = task(4, Some("ci"), &["work"]);
    view(&mut prerequisite, "view_rank", "30");
    let mut unranked_high = task(5, Some("ci"), &["work"]);
    unranked_high.set_priority(Some("H"));
    let unranked = task(6, Some("ci"), &["work"]);
    let context = vec![task(1, Some("ci"), &["view"]), unranked, issue, handoff, prerequisite, unranked_high];
    let rows = arrange(context.clone(), &ViewOptions::default());
    assert_eq!(ids(&rows), vec![1, 3, 4, 2, 5, 6]);
    let shown = presentation(&rows, &context);
    assert_eq!(shown[1].kind, "handoff");
    assert_eq!(shown[2].state, "ready");
    assert_eq!(shown[3].kind, "issue");
    assert_eq!(shown[3].state, "blocked");
    assert_eq!(start_shortcut(&rows[3]), Some(1));
    assert_eq!(rows[3].depends(), Some(&vec![Uuid::from_u128(4)]));
  }

  #[test]
  fn blocked_tag_stays_visible_and_unready_and_other_sorts_ignore_ranks() {
    let mut blocked = task(2, Some("ci"), &["work", "BLOCKED"]);
    blocked.uda_mut().insert("view_rank".into(), UDAValue::U64(10));
    *blocked.description_mut() = "Zulu".into();
    let mut ready = task(3, Some("ci"), &["work"]);
    ready.uda_mut().insert("view_rank".into(), UDAValue::U64(20));
    *ready.description_mut() = "Alpha".into();
    let context = vec![task(1, Some("ci"), &["view"]), ready, blocked];
    let rows = arrange(context.clone(), &ViewOptions::default());
    assert_eq!(ids(&rows), vec![1, 2, 3]);
    assert_eq!(presentation(&rows, &context)[1].state, "blocked");
    assert_eq!(presentation(&rows, &context)[2].state, "ready");
    assert_eq!(
      ids(&arrange(
        context.clone(),
        &ViewOptions {
          sort: Sort::Title,
          ..Default::default()
        }
      )),
      vec![1, 3, 2]
    );
    assert_eq!(
      ids(&arrange(
        context,
        &ViewOptions {
          ready_only: true,
          ..Default::default()
        }
      )),
      vec![1, 3]
    );
  }

  #[test]
  fn projects_arrangement_sorts_headers_and_keeps_each_uuid_once_with_orphans_last() {
    let header = task(1, Some("Zulu"), &["view"]);
    let child = task(2, Some("Zulu"), &["work"]);
    let rows = arrange(
      vec![
        task(5, None, &["piotr"]),
        child.clone(),
        header.clone(),
        task(3, Some("alpha"), &["view"]),
        task(4, Some("alpha"), &["piotr"]),
        header,
        child,
        task(6, Some("Zulu"), &["view"]),
        task(7, Some("admin"), &["piotr"]),
      ],
      &ViewOptions::default(),
    );
    assert_eq!(ids(&rows), vec![3, 4, 1, 2, 6, 5, 7]);
    assert_eq!(rows.len(), rows.iter().map(|task| *task.uuid()).collect::<HashSet<_>>().len());
  }

  fn settings(lines: &[(&str, &str)]) -> (impl Fn(&str) -> Option<String>, Vec<String>) {
    let map: HashMap<String, String> = lines.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let mut defined = Vec::new();
    for (key, _) in lines {
      if let Some((name, _)) = key.strip_prefix(REPORT_PREFIX).and_then(|rest| rest.split_once('.'))
        && !defined.contains(&name.to_string())
      {
        defined.push(name.to_string());
      }
    }
    (move |key: &str| map.get(key).cloned(), defined)
  }

  fn names(reports: &[Report]) -> Vec<&str> {
    reports.iter().map(|report| report.name.as_str()).collect()
  }

  #[test]
  fn reports_default_to_builtins_with_next_first() {
    let (get, defined) = settings(&[]);
    let reports = reports(get, &defined);
    assert_eq!(
      names(&reports),
      ["next", "todo", "active", "handoffs", "ready", "due", "prio", "newest", "oldest"]
    );
    assert_eq!(reports[0].options, ViewOptions::default(), "next is today's default view");
    let sort = |name: &str| reports.iter().find(|r| r.name == name).unwrap().options.sort;
    // The former sort keys 1 to 5 keep their order as reports.
    for (name, expected) in [
      ("next", Sort::Execution),
      ("due", Sort::Due),
      ("prio", Sort::Priority),
      ("newest", Sort::Newest),
      ("oldest", Sort::Oldest),
    ] {
      assert_eq!(sort(name), expected, "{name}");
    }
    let handoffs = reports.iter().find(|r| r.name == "handoffs").unwrap();
    assert!(handoffs.options.handoffs_only && handoffs.options.hide_empty);
    assert!(reports.iter().find(|r| r.name == "ready").unwrap().options.ready_only);
    assert!(reports.iter().all(|r| !r.collapsed));
  }

  #[test]
  fn reports_read_settings_list_and_custom_names() {
    let (get, defined) = settings(&[
      (REPORT_LIST, "todo, mine ,next,todo,,bad name"),
      ("uda.taskwarrior-tui.projects.report.todo.hide-status", "Backlog"),
      ("uda.taskwarrior-tui.projects.report.mine.sort", "due"),
      ("uda.taskwarrior-tui.projects.report.mine.hide-empty", "yes"),
      ("uda.taskwarrior-tui.projects.report.mine.hide-local", "on"),
      ("uda.taskwarrior-tui.projects.report.mine.collapsed", "true"),
      ("uda.taskwarrior-tui.projects.report.next.sort", "sideways"),
      ("uda.taskwarrior-tui.projects.report.next.ready-only", "maybe"),
    ]);
    let reports = reports(get, &defined);
    assert_eq!(names(&reports), ["todo", "mine", "next"], "the list fixes order, once each");
    assert_eq!(reports[0].options.hide_statuses, ["Backlog"]);
    let mine = &reports[1];
    assert_eq!(mine.options.sort, Sort::Due);
    assert!(mine.options.hide_empty && mine.options.hide_local && mine.collapsed);
    assert_eq!(reports[2].options, ViewOptions::default(), "invalid values keep the defaults");

    let (get, defined) = settings(&[("uda.taskwarrior-tui.projects.report.mine.handoffs-only", "yes")]);
    let reports = super::reports(get, &defined);
    assert_eq!(reports.last().unwrap().name, "mine", "unlisted custom reports follow the built-ins");
    assert!(reports.last().unwrap().options.handoffs_only);
    assert_eq!(reports[0].name, "next");
  }

  fn header(id: u128, project: &str, key: &str) -> Task {
    let mut header = task(id, Some(project), &["view"]);
    view(&mut header, "view_key", key);
    view(&mut header, "view_kind", "project");
    header
  }

  #[test]
  fn report_filters_hide_rows_and_projects() {
    let mut triage = task(11, Some("ci"), &["view"]);
    view(&mut triage, "view_key", "issue:CI-1");
    view(&mut triage, "view_state", "triage");
    let mut started = task(12, Some("ci"), &["work"]);
    view(&mut started, "linear_status", "In Progress");
    let mut blocked = task(21, Some("docs"), &["work"]);
    depends(&mut blocked, &[22]);
    let mut prerequisite = task(22, Some("other"), &["work"]);
    view(&mut prerequisite, "linear_status", "Backlog");
    let context = vec![
      header(1, "ci", "standing:ci"),
      triage,
      started,
      header(2, "docs", "standing:docs"),
      blocked,
      header(3, "empty", "standing:empty"),
      header(4, "ops", "local:ops"),
      task(41, Some("ops"), &["piotr"]),
      prerequisite,
    ];
    let builtin = |name: &str| builtin_reports().into_iter().find(|r| r.name == name).unwrap().options;
    assert_eq!(
      ids(&arrange(context.clone(), &ViewOptions::default())),
      vec![1, 11, 12, 2, 21, 3, 4, 41, 22]
    );

    let todo = arrange(context.clone(), &builtin("todo"));
    assert_eq!(
      ids(&todo),
      vec![1, 12, 2, 21, 3, 4, 41],
      "Triage and Backlog rows hide, whatever the case"
    );
    let shown = presentation(&arrange(context.clone(), &ViewOptions::default()), &context);
    assert_eq!(shown[1].linear_status, "triage", "a mirror row's view_state is its Linear status");
    assert_eq!(shown[2].linear_status, "In Progress");
    assert_eq!(
      shown[0].linear_status, "",
      "a header's view_state is the project state, shown under State"
    );

    let active = arrange(context.clone(), &builtin("active"));
    assert_eq!(
      ids(&active),
      vec![1, 11, 12, 2, 21, 4, 41, 22],
      "an empty project hides; a blocked step keeps docs"
    );

    let handoffs = arrange(context.clone(), &builtin("handoffs"));
    assert_eq!(ids(&handoffs), vec![4, 41], "only projects with handoffs remain");

    let local = arrange(
      context.clone(),
      &ViewOptions {
        hide_local: true,
        ..Default::default()
      },
    );
    assert_eq!(
      ids(&local),
      vec![1, 11, 12, 2, 21, 3, 22],
      "a hidden local project takes its rows, not Unsorted"
    );
  }

  #[test]
  fn jumped_names_only_the_step_that_moved_ahead() {
    let ranked = |id: u128, project: &str, rank: f64| {
      let mut step = task(id, Some(project), &["work"]);
      step.uda_mut().insert("view_rank".into(), UDAValue::F64(rank));
      step
    };
    let queue = |ranks: [f64; 3], other: f64| {
      vec![
        header(1, "ci", "standing:ci"),
        ranked(2, "ci", ranks[0]),
        ranked(3, "ci", ranks[1]),
        ranked(4, "ci", ranks[2]),
        header(5, "docs", "standing:docs"),
        ranked(6, "docs", other),
        ranked(7, "loose", 1.0),
      ]
    };
    let before = queue([10.0, 20.0, 30.0], 10.0);
    // Step 4 moves ahead of 2 and 3; nothing else changed position.
    assert_eq!(jumped(&before, &queue([10.0, 20.0, 5.0], 10.0)), vec![Uuid::from_u128(4)]);
    // Step 2 falls behind the others: the others did not jump.
    assert!(jumped(&before, &queue([40.0, 20.0, 30.0], 10.0)).is_empty());
    // A renumber keeps the order, and a lower rank that passes no one is quiet.
    assert!(jumped(&before, &queue([1.0, 2.0, 3.0], 10.0)).is_empty());
    assert!(jumped(&before, &queue([5.0, 20.0, 30.0], 1.0)).is_empty());
    // Rows new to the export or outside a project never count.
    assert!(jumped(&[], &before).is_empty());
    let mut moved = queue([10.0, 20.0, 30.0], 10.0);
    moved[6].uda_mut().insert("view_rank".into(), UDAValue::F64(0.5));
    assert!(jumped(&before, &moved).is_empty());
  }

  #[test]
  fn action_names_what_s_runs() {
    let mut triage = task(1, Some("ci"), &["view"]);
    view(&mut triage, "view_key", "issue:CI-1");
    view(&mut triage, "linear_status", "Triage");
    assert_eq!(action(&triage), "triage");
    view(&mut triage, "linear_status", "Todo");
    assert_eq!(action(&triage), "open");
    let mut handoff = task(2, Some("ci"), &["piotr"]);
    assert_eq!(action(&handoff), "start");
    view(&mut handoff, "kind", "pr");
    assert_eq!(action(&handoff), "pr");
    handoff.set_start(Some(date(2020)));
    assert_eq!(action(&handoff), "stop");
    assert_eq!(action(&header(3, "ci", "standing:ci")), "brief");
  }

  #[test]
  fn presentation_uses_columns_without_changing_native_task_data() {
    let mut linked = task(2, Some("admin"), &["piotr"]);
    view(&mut linked, "view_project", "ai");
    let mut unresolved = task(4, Some("ai"), &["piotr"]);
    view(&mut unresolved, "view_project", "!unassigned");
    let context = vec![task(1, Some("ai"), &["view"]), linked, task(3, Some("other"), &["work"]), unresolved];
    let arranged = arrange(context.clone(), &ViewOptions::default());
    let rows = presentation(&arranged, &context);
    assert_eq!(rows[0].project, "ai");
    assert_eq!(rows[0].kind, "project");
    assert_eq!(rows[0].description, "task 1");
    assert_eq!(rows[1].project, "");
    assert_eq!(rows[1].kind, "handoff");
    assert_eq!(rows[0].action, "brief", "s on a header opens its brief");
    assert_eq!(rows[1].action, "start", "a handoff without a kind starts");
    assert_eq!(rows[1].description, "  ↳ task 2");
    assert_eq!(arranged[1].project().map(String::as_str), Some("admin"));
    assert_eq!(arranged[1].description(), "task 2");
    assert!(rows[2..].iter().all(|row| row.project == "Unsorted"));
    assert_eq!(rows[2].action, "start");
    assert_eq!(arranged[3].project().map(String::as_str), Some("ai"));
  }

  #[test]
  fn project_sort_modes_use_their_requested_primary_order() {
    let mut alpha = task(2, Some("ci"), &["work"]);
    *alpha.description_mut() = "Alpha".into();
    alpha.set_priority(Some("M"));
    *alpha.entry_mut() = date(2022);
    alpha.set_due(Some(date(2024)));
    let mut bravo = task(3, Some("ci"), &["work"]);
    *bravo.description_mut() = "Bravo".into();
    bravo.set_priority(Some("H"));
    *bravo.entry_mut() = date(2024);
    bravo.set_due(Some(date(2027)));
    let mut mike = task(4, Some("ci"), &["work"]);
    *mike.description_mut() = "Mike".into();
    mike.set_priority(Some("L"));
    *mike.entry_mut() = date(2021);
    let mut zulu = task(5, Some("ci"), &["work"]);
    *zulu.description_mut() = "Zulu".into();
    zulu.set_priority(Some("H"));
    *zulu.entry_mut() = date(2020);
    zulu.set_due(Some(date(2026)));
    let context = vec![task(1, Some("ci"), &["view"]), alpha, bravo, mike, zulu];
    for (sort, expected) in [
      (Sort::Execution, vec![1, 5, 3, 2, 4]),
      (Sort::Due, vec![1, 2, 5, 3, 4]),
      (Sort::Priority, vec![1, 5, 3, 2, 4]),
      (Sort::Newest, vec![1, 3, 2, 4, 5]),
      (Sort::Oldest, vec![1, 5, 4, 2, 3]),
      (Sort::Title, vec![1, 2, 3, 4, 5]),
    ] {
      assert_eq!(
        ids(&arrange(context.clone(), &ViewOptions { sort, ..Default::default() })),
        expected,
        "{sort:?}"
      );
    }
  }

  #[test]
  fn all_project_sorts_keep_dependencies_and_hidden_prerequisite_chains_in_order() {
    let mut dependent = task(2, Some("ci"), &["piotr"]);
    *dependent.description_mut() = "Alpha".into();
    dependent.set_priority(Some("H"));
    dependent.set_due(Some(date(2025)));
    depends(&mut dependent, &[3]);
    let mut bridge = task(3, Some("ci"), &["work"]);
    depends(&mut bridge, &[4]);
    let mut prerequisite = task(4, Some("ci"), &["piotr"]);
    *prerequisite.description_mut() = "Zulu".into();
    prerequisite.set_priority(Some("L"));
    let context = vec![task(1, Some("ci"), &["view"]), dependent, bridge, prerequisite];
    for sort in SORTS {
      let arranged = arrange(context.clone(), &ViewOptions { sort, ..Default::default() });
      assert_eq!(ids(&arranged), vec![1, 4, 3, 2], "{sort:?}");
      let arranged = arrange(
        context.clone(),
        &ViewOptions {
          sort,
          handoffs_only: true,
          ..Default::default()
        },
      );
      assert_eq!(ids(&arranged), vec![1, 4, 2], "{sort:?}");
      assert_eq!(presentation(&arranged, &context)[2].state, "blocked");
    }
  }

  #[test]
  fn filters_keep_headers_and_use_full_context_for_readiness_and_cycles() {
    let mut blocked = task(3, Some("ci"), &["piotr"]);
    depends(&mut blocked, &[2]);
    let mut waiting = task(5, Some("ci"), &["piotr"]);
    *waiting.status_mut() = TaskStatus::Waiting;
    let mut missing = task(6, Some("ci"), &["piotr"]);
    depends(&mut missing, &[999]);
    let mut cycle = task(7, Some("ci"), &["piotr"]);
    depends(&mut cycle, &[8]);
    let mut hidden_cycle = task(8, Some("ci"), &["work"]);
    depends(&mut hidden_cycle, &[7]);
    let mut running = task(9, Some("ci"), &["piotr"]);
    running.set_start(Some(chrono::Utc::now().naive_utc()));
    let context = vec![
      task(1, Some("ci"), &["view"]),
      task(2, Some("ci"), &["work"]),
      blocked,
      task(4, Some("ci"), &["piotr"]),
      waiting,
      missing,
      cycle,
      hidden_cycle,
      running,
      task(10, Some("empty"), &["view"]),
    ];
    let ready = arrange(
      context.clone(),
      &ViewOptions {
        ready_only: true,
        ..Default::default()
      },
    );
    assert_eq!(ids(&ready), vec![1, 2, 4, 9, 10]);
    let handoffs = arrange(
      context.clone(),
      &ViewOptions {
        handoffs_only: true,
        ..Default::default()
      },
    );
    assert_eq!(
      ids(&handoffs).into_iter().collect::<HashSet<_>>(),
      HashSet::from([1, 3, 4, 5, 6, 7, 9, 10])
    );
    let states: HashMap<_, _> = handoffs
      .iter()
      .zip(presentation(&handoffs, &context))
      .map(|(task, row)| (task.uuid().as_u128(), row.state))
      .collect();
    assert_eq!(states[&3], "blocked");
    assert_eq!(states[&5], "waiting");
    assert_eq!(states[&6], "missing dep");
    assert_eq!(states[&7], "cycle");
    assert_eq!(states[&9], "running");
    let both = arrange(
      context,
      &ViewOptions {
        ready_only: true,
        handoffs_only: true,
        ..Default::default()
      },
    );
    assert_eq!(ids(&both), vec![1, 4, 9, 10]);
  }

  #[test]
  fn completed_dependencies_in_hidden_context_stay_satisfied() {
    let mut done = task(2, Some("outside"), &["work"]);
    *done.status_mut() = TaskStatus::Completed;
    let mut handoff = task(3, Some("ci"), &["piotr"]);
    depends(&mut handoff, &[2]);
    let context = vec![task(1, Some("ci"), &["view"]), done, handoff];
    let arranged = arrange(
      context.clone(),
      &ViewOptions {
        ready_only: true,
        handoffs_only: true,
        ..Default::default()
      },
    );
    assert_eq!(ids(&arranged), vec![1, 3]);
    assert_eq!(presentation(&arranged, &context)[1].state, "ready");
  }

  #[test]
  fn presentation_cleans_only_known_generated_descriptions_and_prefers_structured_fields() {
    let mut header = task(1, Some("ci"), &["view"]);
    view(&mut header, "view_key", "standing:ci-id");
    *header.description_mut() = "standing · Active project context · source unavailable".into();
    let mut source = task(2, Some("ci"), &["view"]);
    view(&mut source, "view_key", "issue:DEVOPS-123");
    *source.description_mut() = "no task · DEVOPS-123 In Review · Fix report".into();
    let mut handoff = task(3, Some("ci"), &["piotr", "view"]);
    view(&mut handoff, "view_key", "issue:DEVOPS-456");
    view(&mut handoff, "view_title", "Do not replace real prose");
    *handoff.description_mut() = "no task · DEVOPS-456 Todo · Keep my exact words".into();
    let mut plain = task(4, Some("ci"), &["work"]);
    *plain.description_mut() = "standing · My literal title".into();
    let mut malformed = task(5, Some("ci"), &["view"]);
    view(&mut malformed, "view_key", "issue:DEVOPS-789");
    *malformed.description_mut() = "no task · A personal note · Keep this".into();
    let context = vec![header, source, handoff, plain, malformed];
    let rows = presentation(&context, &context);
    assert_eq!(rows[0].state, "source unavailable");
    assert_eq!(rows[0].description, "Active project context");
    assert_eq!(rows[1].kind, "issue");
    assert_eq!(rows[1].state, "ready");
    assert_eq!(rows[1].linear, "DEVOPS-123");
    assert_eq!(rows[1].description, "  ↳ Fix report");
    assert_eq!(rows[2].kind, "handoff");
    assert_eq!(rows[2].description, "  ↳ no task · DEVOPS-456 Todo · Keep my exact words");
    assert_eq!(rows[3].description, "  ↳ standing · My literal title");
    assert_eq!(rows[4].description, "  ↳ no task · A personal note · Keep this");

    let mut structured = context[1].clone();
    view(&mut structured, "view_kind", "task");
    view(&mut structured, "view_title", "Current clean title");
    view(&mut structured, "view_state", "Triage");
    view(&mut structured, "view_issue", "DEVOPS-999");
    let rows = presentation(&[structured], &context);
    assert_eq!(rows[0].kind, "task");
    assert_eq!(rows[0].state, "ready");
    assert_eq!(rows[0].linear, "DEVOPS-999");
    assert_eq!(rows[0].description, "  ↳ Current clean title");
  }

  #[test]
  fn generated_workflow_state_is_separate_from_dependency_readiness_filter() {
    let mut source = task(2, Some("ci"), &["view"]);
    view(&mut source, "view_key", "issue:DEVOPS-123");
    view(&mut source, "view_state", "Triage");
    let mut blocked = source.clone();
    *blocked.uuid_mut() = Uuid::from_u128(3);
    depends(&mut blocked, &[2]);
    let context = vec![task(1, Some("ci"), &["view"]), source, blocked];
    let arranged = arrange(
      context.clone(),
      &ViewOptions {
        ready_only: true,
        ..Default::default()
      },
    );
    assert_eq!(ids(&arranged), vec![1, 2]);
    assert_eq!(presentation(&arranged, &context)[1].state, "ready");
  }

  #[test]
  fn legacy_header_metadata_moves_into_columns_without_parsing_actual_task_prose() {
    let description = "standing · ENGAI-137 Research agents (with tests) (In Review)";
    let mut header = task(1, Some("ai"), &["view"]);
    view(&mut header, "view_key", "standing:ai-id");
    *header.description_mut() = description.into();
    for unavailable in [false, true] {
      *header.description_mut() = format!("{description}{}", if unavailable { " · source unavailable" } else { "" });
      let rows = presentation(&[header.clone()], &[]);
      assert_eq!(rows[0].description, "Research agents (with tests)");
      assert_eq!(rows[0].linear, "ENGAI-137");
      assert_eq!(rows[0].state, if unavailable { "source unavailable" } else { "In Review" });
    }

    view(&mut header, "view_title", "Structured title");
    view(&mut header, "view_issue", "ENGAI-200");
    view(&mut header, "view_state", "Structured state");
    let rows = presentation(&[header.clone()], &[]);
    assert_eq!(rows[0].description, "Structured title");
    assert_eq!(rows[0].linear, "ENGAI-200");
    assert_eq!(rows[0].state, "Structured state");

    for tags in [&["work"][..], &["piotr"][..], &["view", "piotr"][..]] {
      let mut actual = task(2, Some("ai"), tags);
      *actual.description_mut() = description.into();
      let rows = presentation(&[actual], &[]);
      assert_eq!(rows[0].description, description);
      assert_eq!(rows[0].linear, "");
      assert_eq!(rows[0].state, "ready");
    }

    let mut unrecognized = task(3, Some("ai"), &["view"]);
    view(&mut unrecognized, "view_key", "standing:ai-id");
    *unrecognized.description_mut() = "Research ENGAI-137 and discuss (In Review)".into();
    let rows = presentation(&[unrecognized.clone()], &[]);
    assert_eq!(rows[0].description, unrecognized.description().as_str());
    assert_eq!(rows[0].linear, "");
    assert_eq!(rows[0].state, "");
  }

  #[test]
  fn groups_regular_work_and_handoffs_without_losing_orphans() {
    let mut work = task(3, Some("ci"), &["work"]);
    work.set_priority(Some("H"));
    let rows = group(vec![
      task(1, Some("ci"), &["view"]),
      task(2, Some("ci"), &["piotr"]),
      work,
      task(4, Some("admin"), &["piotr"]),
      task(5, None, &["piotr"]),
      task(6, Some("other"), &["work"]),
      task(7, Some("ci.child"), &["piotr"]),
      task(8, Some("ci-other"), &["piotr"]),
    ]);
    assert_eq!(ids(&rows), vec![1, 3, 2, 4, 5, 6, 7, 8]);
    let prefixes = descriptions(&rows);
    assert!(prefixes[1].starts_with("  ↳ [ready]"));
    for (i, project) in [(3, "admin"), (4, "no project"), (5, "other"), (6, "ci.child"), (7, "ci-other")] {
      assert!(prefixes[i].starts_with(&format!("[ungrouped: {project}]")), "{}", prefixes[i]);
    }
  }

  #[test]
  fn local_project_header_groups_native_members() {
    let mut header = task(1, Some("operations"), &["view"]);
    view(&mut header, "view_key", "local:operations");
    view(&mut header, "view_kind", "project");
    let first = task(2, Some("operations"), &["work"]);
    let second = task(3, Some("operations"), &["work"]);
    let rows = group(vec![second, header, first]);
    assert_eq!(ids(&rows), vec![1, 2, 3]);
    assert!(
      rows
        .iter()
        .all(|row| project_group(row, &rows) == ProjectGroup::Named("operations".into()))
    );
  }

  #[test]
  fn keyed_issue_and_ask_views_remain_actions_under_standing_headers() {
    let mut header = task(1, Some("ci"), &["view"]);
    header.uda_mut().insert("view_key".into(), UDAValue::Str("standing:project-id".into()));
    let mut issue = task(2, Some("ci"), &["view", "work"]);
    issue.uda_mut().insert("view_key".into(), UDAValue::Str("issue:issue-id".into()));
    let mut ask = task(3, Some("ci"), &["view", "piotr"]);
    ask.uda_mut().insert("view_key".into(), UDAValue::Str("ask:ask-id".into()));
    assert!(is_overview(&header));
    assert!(!is_overview(&issue));
    assert!(!is_overview(&ask));
    let rows = group(vec![ask, issue, header]);
    assert_eq!(ids(&rows), vec![1, 2, 3]);
    assert_eq!(descriptions(&rows), vec!["", "  ↳ [ready] ", "  ↳ [ready] "]);
  }

  #[test]
  fn source_views_open_sources_even_with_retained_runner_metadata() {
    use task_hookrs::annotation::Annotation;

    for key in ["issue:source-id", "ask:source-id"] {
      let mut source = task(1, Some("ci"), &["view", "ACTIVE"]);
      source.uda_mut().insert("view_key".into(), UDAValue::Str(key.into()));
      source.uda_mut().insert("linear_issue_id".into(), UDAValue::Str("DEVOPS-123".into()));
      source.set_start(Some(chrono::Utc::now().naive_utc()));
      source.set_annotations::<_, Annotation>(Some(vec![Annotation::new(source.entry().clone(), "tmux session test-worker".into())]));
      assert_eq!(start_shortcut(&source), Some(1));
      source.tags_mut().as_mut().unwrap().push("piotr".into());
      assert_eq!(start_shortcut(&source), None, "handoff hook keeps precedence");
    }
  }

  #[test]
  fn explicit_project_membership_preserves_category_and_never_falls_back_on_a_miss() {
    let mut linked = task(2, Some("admin"), &["piotr"]);
    linked.uda_mut().insert("view_project".into(), UDAValue::Str("ci".into()));
    let mut empty = task(3, Some("ci"), &["work"]);
    empty.uda_mut().insert("view_project".into(), UDAValue::Str(String::new()));
    let mut unmatched = task(4, Some("ci"), &["piotr"]);
    unmatched.uda_mut().insert("view_project".into(), UDAValue::Str("ci-other".into()));
    let rows = group(vec![linked, empty, unmatched, task(1, Some("ci"), &["view"])]);
    assert_eq!(ids(&rows), vec![4, 1, 2, 3]);
    assert_eq!(rows[2].project().map(String::as_str), Some("admin"));
    assert_eq!(rows[2].uda().get("view_project"), Some(&UDAValue::Str("ci".into())));
    assert!(descriptions(&rows)[0].starts_with("[ungrouped: ci-other]"));
  }

  #[test]
  fn explicitly_unassigned_project_prevents_native_project_fallback() {
    let mut unresolved = task(2, Some("ci"), &["piotr"]);
    unresolved.uda_mut().insert("view_project".into(), UDAValue::Str("!unassigned".into()));
    let mut linked = task(3, Some("admin"), &["piotr"]);
    linked.uda_mut().insert("view_project".into(), UDAValue::Str("ci".into()));
    let absent = task(4, Some("ci"), &["work"]);
    let mut empty = task(5, Some("ci"), &["work"]);
    empty.uda_mut().insert("view_project".into(), UDAValue::Str(String::new()));

    assert_eq!(membership(&unresolved), None);
    let rows = group(vec![unresolved, linked, absent, empty, task(1, Some("ci"), &["view"])]);
    assert_eq!(ids(&rows), vec![2, 1, 3, 4, 5]);
    assert_eq!(rows[0].project().map(String::as_str), Some("ci"));
    assert_eq!(rows[0].uda().get("view_project"), Some(&UDAValue::Str("!unassigned".into())));
    assert_eq!(descriptions(&rows)[0], "[unresolved project] [ready] ");
    assert!(descriptions(&rows)[2..].iter().all(|prefix| prefix.starts_with("  ↳ ")));
  }

  #[test]
  fn duplicate_or_missing_headers_never_duplicate_or_drop_actions() {
    let rows = vec![
      task(2, Some("ci"), &["piotr"]),
      task(3, None, &["view"]),
      task(4, Some("admin"), &["piotr"]),
    ];
    assert_eq!(ids(&group(rows)), vec![2, 3, 4]);
    let header = task(1, Some("ci"), &["view"]);
    let rows = group(vec![
      header.clone(),
      task(2, Some("ci"), &["work"]),
      task(3, Some("ci"), &["view"]),
      task(4, Some("admin"), &["piotr"]),
      task(5, Some("admin"), &["view"]),
      header,
    ]);
    assert_eq!(ids(&rows), vec![1, 2, 3, 4, 5]);
    assert!(descriptions(&rows)[3].starts_with("[ungrouped: admin]"));
  }

  #[test]
  fn linear_status_has_its_own_field_and_plain_tasks_stay_unchanged() {
    let mut issue = task(2, Some("ci"), &["work"]);
    issue.uda_mut().insert("linear_issue_id".into(), UDAValue::Str("DEVOPS-1688".into()));
    issue.uda_mut().insert("linear_status".into(), UDAValue::Str("Parked".into()));
    let plain = task(3, Some("ci"), &["work"]);
    let rows = presentation(&[issue, plain], &[]);
    assert_eq!(rows[0].linear, "DEVOPS-1688");
    assert_eq!(rows[0].linear_status, "Parked");
    assert_eq!(rows[1].linear_status, "");
    assert_eq!(rows[1].linear, "");
  }

  #[test]
  fn prerequisites_precede_higher_priority_dependents_and_keep_real_links() {
    let mut first = task(2, Some("ci"), &["work"]);
    first.set_priority(Some("L"));
    let mut second = task(3, Some("ci"), &["piotr"]);
    second.set_priority(Some("M"));
    depends(&mut second, &[2]);
    let mut third = task(4, Some("ci"), &["work"]);
    third.set_priority(Some("H"));
    depends(&mut third, &[3]);
    let rows = group(vec![third, second, task(1, Some("ci"), &["view"]), first]);
    assert_eq!(ids(&rows), vec![1, 2, 3, 4]);
    assert_eq!(rows[3].depends(), Some(&vec![Uuid::from_u128(3)]));
    assert!(rows[1].depends().is_none());
    assert_eq!(descriptions(&rows)[2], "  ↳ [blocked: 2] ");
  }

  #[test]
  fn cycles_and_absent_dependencies_stay_visible_and_are_never_ready() {
    let mut first = task(2, Some("ci"), &["work"]);
    depends(&mut first, &[3]);
    let mut second = task(3, Some("ci"), &["piotr"]);
    depends(&mut second, &[2]);
    let mut absent = task(4, Some("ci"), &["piotr"]);
    depends(&mut absent, &[999]);
    let mut downstream = task(5, Some("ci"), &["work"]);
    depends(&mut downstream, &[2]);
    let rows = group(vec![task(1, Some("ci"), &["view"]), second, absent, downstream, first]);
    assert_eq!(rows.len(), 5);
    let prefixes: HashMap<_, _> = rows
      .iter()
      .zip(descriptions(&rows))
      .map(|(t, prefix)| (t.uuid().as_u128(), prefix))
      .collect();
    assert!(prefixes[&2].contains("dependency cycle"));
    assert!(prefixes[&3].contains("dependency cycle"));
    assert!(prefixes[&4].contains(&format!("dependency not in report: {}", Uuid::from_u128(999))));
    assert_eq!(prefixes[&5], "  ↳ [blocked: 2] ");
    assert!(prefixes.values().all(|prefix| !prefix.contains("ready")));
  }

  #[test]
  fn dependency_state_uses_other_projects_without_reparenting() {
    let mut dependent = task(2, Some("ci"), &["work"]);
    depends(&mut dependent, &[4]);
    let rows = group(vec![
      task(1, Some("ci"), &["view"]),
      dependent,
      task(3, Some("agents"), &["view"]),
      task(4, Some("agents"), &["piotr"]),
    ]);
    assert_eq!(ids(&rows), vec![1, 2, 3, 4]);
    assert_eq!(descriptions(&rows)[1], "  ↳ [blocked: 4] ");
  }

  #[test]
  fn ready_priority_due_newest_and_uuid_provide_deterministic_fallback() {
    let mut high = task(2, Some("ci"), &["work"]);
    high.set_priority(Some("H"));
    let mut due = task(3, Some("ci"), &["piotr"]);
    due.set_due(Some(Date::from(
      NaiveDate::from_ymd_opt(2025, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap(),
    )));
    let mut newer = task(4, Some("ci"), &["work"]);
    *newer.entry_mut() = Date::from(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap());
    let mut blocked_high = task(7, Some("ci"), &["piotr"]);
    blocked_high.set_priority(Some("H"));
    depends(&mut blocked_high, &[999]);
    let mut waiting = task(8, Some("ci"), &["work"]);
    *waiting.status_mut() = TaskStatus::Waiting;
    let rows = vec![
      task(1, Some("ci"), &["view"]),
      blocked_high,
      waiting,
      task(6, Some("ci"), &["work"]),
      newer,
      task(5, Some("ci"), &["work"]),
      due,
      high,
    ];
    let expected = vec![1, 2, 3, 4, 5, 6, 7, 8];
    assert_eq!(ids(&group(rows.clone())), expected);
    assert_eq!(ids(&group(rows.into_iter().rev().collect())), expected);
  }

  #[test]
  fn completed_dependencies_are_satisfied() {
    let mut prerequisite = task(2, Some("ci"), &["work"]);
    *prerequisite.status_mut() = TaskStatus::Completed;
    let mut action = task(3, Some("ci"), &["piotr"]);
    action.set_priority(Some("H"));
    depends(&mut action, &[2]);
    let rows = group(vec![task(1, Some("ci"), &["view"]), prerequisite, action]);
    assert_eq!(ids(&rows), vec![1, 3, 2]);
    assert_eq!(descriptions(&rows)[1], "  ↳ [ready] ");
  }
}
