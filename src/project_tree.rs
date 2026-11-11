use std::{
  cmp::Ordering,
  collections::{HashMap, HashSet},
  sync::LazyLock,
};

use regex::Regex;
use task_hookrs::{status::TaskStatus, task::Task, uda::UDAValue};
use uuid::Uuid;

pub fn has_tag(task: &Task, tag: &str) -> bool {
  task.tags().as_ref().is_some_and(|tags| tags.iter().any(|t| t == tag))
}

// Source issue/ask rows also carry +view. Only standing project rows are
// overviews; missing keys preserve the earlier project's +view convention.
pub fn is_overview(task: &Task) -> bool {
  has_tag(task, "view")
    && match task.uda().get("view_key") {
      Some(UDAValue::Str(key)) if !key.is_empty() => key.starts_with("standing:"),
      None | Some(UDAValue::Str(_)) => true,
      _ => false,
    }
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

// Administrative work is global, even when a standing admin row exists.
fn project(task: &Task) -> Option<&str> {
  task.project().map(String::as_str).filter(|p| !p.is_empty() && *p != "admin")
}

fn membership(task: &Task) -> Option<&str> {
  match task.uda().get("view_project") {
    Some(UDAValue::Str(project)) if !project.is_empty() => Some(project),
    _ => project(task),
  }
}

fn headers(tasks: &[Task]) -> HashSet<&str> {
  tasks.iter().filter(|t| is_overview(t)).filter_map(project).collect()
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
fn execution_order(mut tasks: Vec<Task>, unready: &HashSet<Uuid>) -> Vec<Task> {
  tasks.sort_by(|a, b| {
    unready
      .contains(a.uuid())
      .cmp(&unready.contains(b.uuid()))
      .then_with(|| fallback_order(a, b))
  });
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
      let location = if membership(task).is_some_and(|p| projects.contains(p)) {
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
