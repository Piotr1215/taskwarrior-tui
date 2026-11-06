use std::{
  fs, io,
  path::{Path, PathBuf},
};
#[cfg(unix)]
use std::{
  fs::OpenOptions,
  io::Write,
  os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
  sync::atomic::{AtomicU64, Ordering},
};
use uuid::Uuid;

#[cfg(unix)]
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub struct SelectionPublisher {
  path: Option<PathBuf>,
  published: Option<Option<Uuid>>,
}

impl SelectionPublisher {
  pub fn from_environment() -> Self {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR");
    let pane = std::env::var_os("TMUX_PANE");
    Self::new(runtime.as_deref().map(Path::new), pane.as_deref().and_then(|p| p.to_str()))
  }

  pub(crate) fn new(runtime: Option<&Path>, pane: Option<&str>) -> Self {
    let filename = match pane {
      None => Some("selected".to_string()),
      Some(p) if p.starts_with('%') && p.len() > 1 && p[1..].bytes().all(|c| c.is_ascii_digit()) => Some(format!("selected-{}", &p[1..])),
      _ => None,
    };
    Self {
      path: runtime
        .filter(|p| p.is_absolute())
        .zip(filename)
        .map(|(p, f)| p.join("taskwarrior-tui").join(f)),
      published: None,
    }
  }

  pub fn publish(&mut self, selection: Option<Uuid>) {
    if self.published == Some(selection) {
      return;
    }
    if self.write_selection(selection).is_ok() {
      self.published = Some(selection);
    }
  }

  #[cfg(unix)]
  fn write_selection(&self, selection: Option<Uuid>) -> io::Result<()> {
    let Some(path) = &self.path else { return Ok(()) };
    let dir = path.parent().expect("signal path has a parent");
    if let Ok(metadata) = fs::symlink_metadata(dir)
      && !metadata.file_type().is_dir()
    {
      return Err(io::Error::new(io::ErrorKind::InvalidInput, "signal directory is not a directory"));
    }
    if let Some(uuid) = selection {
      fs::DirBuilder::new()
        .mode(0o700)
        .create(dir)
        .or_else(|e| if e.kind() == io::ErrorKind::AlreadyExists { Ok(()) } else { Err(e) })?;
      if !fs::symlink_metadata(dir)?.file_type().is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "signal directory is not a directory"));
      }
      fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
      let temporary = dir.join(format!(".selected-{}-{}", std::process::id(), NEXT_TEMP.fetch_add(1, Ordering::Relaxed)));
      let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)?;
        writeln!(file, "{uuid}")?;
        fs::rename(&temporary, path)
      })();
      if result.is_err() {
        let _ = fs::remove_file(&temporary);
      }
      result
    } else {
      match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
      }
    }
  }
}

#[cfg(not(unix))]
impl SelectionPublisher {
  fn write_selection(&self, _selection: Option<Uuid>) -> io::Result<()> {
    Ok(())
  }
}

#[cfg(all(test, unix))]
mod tests {
  use super::*;
  use std::os::unix::fs::{MetadataExt, PermissionsExt};

  #[test]
  fn selection_file_tracks_changes_and_removes_empty_selection() {
    let root = std::env::temp_dir().join(format!(
      "tui-selection-test-{}-{}",
      std::process::id(),
      NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let mut publisher = SelectionPublisher::new(Some(&root), Some("%42"));
    let path = root.join("taskwarrior-tui/selected-42");
    let first = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let second = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
    publisher.publish(Some(first));
    assert_eq!(fs::read_to_string(&path).unwrap(), format!("{first}\n"));
    assert_eq!(fs::metadata(path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    let inode = fs::metadata(&path).unwrap().ino();
    publisher.publish(Some(first));
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    publisher.publish(Some(second));
    assert_eq!(fs::read_to_string(&path).unwrap(), format!("{second}\n"));
    publisher.publish(None);
    assert!(!path.exists());
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn paths_require_runtime_and_a_valid_pane() {
    let root = std::env::temp_dir();
    assert_eq!(
      SelectionPublisher::new(Some(&root), None).path.unwrap(),
      root.join("taskwarrior-tui/selected")
    );
    for pane in ["", "%", "%../bad", "42", "%2/3"] {
      assert!(SelectionPublisher::new(Some(&root), Some(pane)).path.is_none());
    }
    assert!(SelectionPublisher::new(None, None).path.is_none());
    assert!(SelectionPublisher::new(Some(Path::new("relative")), None).path.is_none());
  }
}
