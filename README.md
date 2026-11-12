# `taskwarrior-tui`

> [!IMPORTANT]
> [`taskwarrior` v3.x](https://github.com/GothenburgBitFactory/taskwarrior/releases/tag/v3.0.0) may break `taskwarrior-tui` features in unexpected ways. Please file a bug report if you encounter a bug.
>
> taskwarrior-tui [v0.25.4](https://github.com/kdheepak/taskwarrior-tui/releases/tag/v0.25.4) is the last version supporting taskwarrior v2.x as backend.

[![CI](https://github.com/kdheepak/taskwarrior-tui/workflows/CI/badge.svg)](https://github.com/kdheepak/taskwarrior-tui/actions?query=workflow%3ACI)
[![License](https://img.shields.io/github/license/kdheepak/taskwarrior-tui)](https://github.com/kdheepak/taskwarrior-tui/blob/main/LICENSE)
[![Release](https://img.shields.io/github/v/release/kdheepak/taskwarrior-tui)](https://github.com/kdheepak/taskwarrior-tui/releases/latest)
[![Platform](https://img.shields.io/static/v1?label=platform&message=linux-64%20|%20osx-64%20|%20win-32%20|%20win-64&color=lightgrey)](https://github.com/kdheepak/taskwarrior-tui/releases/latest)
[![Rust](https://img.shields.io/github/languages/top/kdheepak/taskwarrior-tui)](https://github.com/kdheepak/taskwarrior-tui)
[![Coverage](https://img.shields.io/coveralls/github/kdheepak/taskwarrior-tui)](https://coveralls.io/github/kdheepak/taskwarrior-tui)
[![Docs](https://img.shields.io/badge/taskwarrior--tui-docs-red)](https://kdheepak.com/taskwarrior-tui)
[![Downloads](https://img.shields.io/github/downloads/kdheepak/taskwarrior-tui/total)](https://github.com/kdheepak/taskwarrior-tui/releases/latest)

A Terminal User Interface (TUI) for [Taskwarrior](https://taskwarrior.org/) that you didn't know you wanted.

### Features

- vim-like navigation
- live filter updates
- add, delete, complete, log tasks
- multiple selection
- tab completion
- colors based on taskwarrior

https://github.com/user-attachments/assets/a3c4f79b-3967-4904-b614-8bbd500c54c2

### Documentation

<details>
<summary>See <a href="https://kdheepak.com/taskwarrior-tui"
class="uri">https://kdheepak.com/taskwarrior-tui</a> for
documentation.</summary>
<p>See <a href="https://kdheepak.com/taskwarrior-tui/installation/"
class="uri">https://kdheepak.com/taskwarrior-tui/installation/</a> for
installation instructions for your platform.</p>
<p>See <a href="https://kdheepak.com/taskwarrior-tui/quick_start/"
class="uri">https://kdheepak.com/taskwarrior-tui/quick_start/</a> to get
started.</p>
<p>See <a href="https://kdheepak.com/taskwarrior-tui/troubleshooting/"
class="uri">https://kdheepak.com/taskwarrior-tui/troubleshooting/</a> for
logs, timing checks, and debugging steps.</p>
<p>See <a href="https://kdheepak.com/taskwarrior-tui/configuration/keys"
class="uri">https://kdheepak.com/taskwarrior-tui/configuration/keys</a>
or <a href="https://kdheepak.com/taskwarrior-tui/configuration/colors/"
class="uri">https://kdheepak.com/taskwarrior-tui/configuration/colors/</a>
for customization options.</p>
</details>

### Installation

Unless otherwise specified, you will need to install the latest version of `taskwarrior` first. See <https://taskwarrior.org/download/> for more information.

Pre-compiled releases are available on the [GitHub repo](https://github.com/kdheepak/taskwarrior-tui):

1. Download the tar.gz file for your OS from [the latest release](https://github.com/kdheepak/taskwarrior-tui/releases/latest).
2. Unzip the tar.gz file
3. Run with `./taskwarrior-tui`.

See <https://kdheepak.com/taskwarrior-tui/installation/> on instructions for using package managers on various platforms.

If you are compiling from source, you'll need the most recent stable rust compiler.

### Configuration

`taskwarrior-tui` uses `taskwarrior`'s `.taskrc` for configuration.

In text fields, Backspace deletes the previous character and Alt+k deletes from
the cursor to the end of the line. Ctrl+h, Ctrl+j, Ctrl+k and Ctrl+l are reserved
for terminal pane navigation. In Projects, Ctrl+o zooms into a project,
O toggles the header-only view and R moves to the next Projects report.
Number keys run the configured shortcuts there as in any other report.
`s` runs the action named in the Action column, and opens the brief on a
project header. A step that moved ahead of another in its project's queue
since the last refresh shows ↑ in State, and the title counts such steps,
until the cursor reaches the row.

Press `S` to sync the selected issue from Linear and refresh the report in
the background. You can keep using the TUI during the sync. The configured
helper receives only the selected task's UUID, regardless of marked rows.
It supports linked tasks and issue rows in Projects. Configure it in `.taskrc`:

```.taskrc
uda.taskwarrior-tui.sync-issue.command=~/bin/sync-linear-issue --task
# Optional: use Ctrl+S instead of S
uda.taskwarrior-tui.keyconfig.sync-issue=Ctrl+s
```

The helper must fetch only the linked issue and update its local task data.
Rows without an issue link show a message.

Projects reports are named view settings, the way Taskwarrior reports name a
filter and sort. They change only what the Projects view shows. The built-ins
are `next` (the default), `todo`, `active`, `handoffs`, `ready`, `due`, `prio`,
`newest` and `oldest`. Change one or add your own in `.taskrc`:

```.taskrc
# Cycle order for R; without it, the built-ins come first.
uda.taskwarrior-tui.projects.reports=next,todo,active,handoffs
# next (rank, then prerequisites), due, prio, newest, oldest or title
uda.taskwarrior-tui.projects.report.todo.sort=next
# Linear states whose rows the report hides
uda.taskwarrior-tui.projects.report.todo.hide-status=Triage,Backlog,Parked
# Hide projects left with no rows, projects that exist only in Taskwarrior
uda.taskwarrior-tui.projects.report.active.hide-empty=yes
uda.taskwarrior-tui.projects.report.active.hide-local=no
# Show only dependency-ready rows, or only handoffs
uda.taskwarrior-tui.projects.report.ready.ready-only=yes
uda.taskwarrior-tui.projects.report.handoffs.handoffs-only=yes
# Start the report with only project headers shown
uda.taskwarrior-tui.projects.report.overview.collapsed=yes
```

### Daily workflow

Use Projects to choose and run the next action with its project context in
view. From Tasks, press `]` to open Projects and `[` to return with the Tasks
filter and selection preserved. You can also press `r` and select `myprojects`.

1. In the morning, spend three minutes choosing an outcome. Press `O` to show
   project headers, select a project and press `b` or `s` to read its overview.
   Press `O` to expand the rows, then `Ctrl+O` to focus on that project.
2. To start work, read a row's Action column and press `b` for its brief.
   Press `s` to run its action: open a source, start work or open a prepared
   handoff. Press `d` after the work is complete or the handoff is acknowledged.
   If configured, `Ctrl+G` gives the assistant the selected context for the
   next action.
3. After an interruption, press `R` until the footer shows `handoffs`. Review
   one waiting action, then cycle back to `next`. The report order is
   configurable, so the number of presses can vary.
4. At the end of work or a session, revisit the project header's overview and
   choose the next concrete row. A project header is not a completed action.

The built-in `todo` report hides Triage, Backlog and Parked rows. `ready` shows
rows whose task prerequisites are met. `active` hides projects without visible
children. Press `Ctrl+O` or `Esc` to leave project zoom.

Keys `0` through `9` run configured shortcuts. Older instructions that use
number keys to sort Projects are obsolete.

See the documentation for more information:

- <https://kdheepak.com/taskwarrior-tui/configuration/keys>
- <https://kdheepak.com/taskwarrior-tui/configuration/colors>
- <https://kdheepak.com/taskwarrior-tui/configuration/advanced/>

<details>
<summary>Here is an example `.taskrc`</summary>

```.taskrc
### taskwarrior configuration options

# taskwarrior's configuration
data.location=.task
verbose=affected,blank,context,edit,header,footnote,label,new-id,project,special,sync,recur
uda.priority.values=H,M,,L
color.alternate=

# taskwarrior-tui reads color attributes from the following to display the same colors of tasks as the CLI
color.tagged=black on rgb444

# Remove age, tags from task next report.
# taskwarrior-tui reads the labels and columns from these options to display tasks the same way taskwarrior does
report.next.labels=ID,Active,Age,Deps,P,Project,Tag,Recur,S,Due,Until,Description,Urg
report.next.columns=id,start.age,entry.age,depends,priority,project,tags,recur,scheduled.countdown,due.relative,until.remaining,description.truncated_count,urgency
report.next.filter=(status:pending or status:waiting) page:limit

### taskwarrior-tui configuration options

uda.taskwarrior-tui.keyconfig.done=x
uda.taskwarrior-tui.keyconfig.delete=d
uda.taskwarrior-tui.task-report.use-alternate-style=false
uda.taskwarrior-tui.shortcuts.1=~/local/bin/task-sync.sh
uda.taskwarrior-tui.report.next.filter=(status:pending or status:waiting)
```

</details>

### References / Resources

If you like `taskwarrior-tui`, please consider donating to

- [`kdheepak`](https://github.com/sponsors/kdheepak)
- [`@GothenburgBitFactory`](https://github.com/sponsors/GothenburgBitFactory)
- and/or a charity of your choice.

<details>
<summary>Additional resources</summary>
<ul>
<li><a href="https://github.com/GothenburgBitFactory/taskwarrior"
class="uri">https://github.com/GothenburgBitFactory/taskwarrior</a></li>
<li><a href="https://github.com/GothenburgBitFactory/libshared"
class="uri">https://github.com/GothenburgBitFactory/libshared</a></li>
<li><a href="https://github.com/GothenburgBitFactory/timewarrior"
class="uri">https://github.com/GothenburgBitFactory/timewarrior</a></li>
<li><a href="https://github.com/fdehau/tui-rs"
class="uri">https://github.com/fdehau/tui-rs</a></li>
<li><a href="https://github.com/tui-rs-revival/ratatui"
class="uri">https://github.com/tui-rs-revival/ratatui</a></li>
<li><a href="https://github.com/crossterm-rs/crossterm/"
class="uri">https://github.com/crossterm-rs/crossterm/</a></li>
<li><a href="https://github.com/async-rs/async-std"
class="uri">https://github.com/async-rs/async-std</a></li>
<li><a href="https://github.com/kkawakam/rustyline"
class="uri">https://github.com/kkawakam/rustyline</a></li>
<li><a href="https://github.com/vit-project/vit"
class="uri">https://github.com/vit-project/vit</a></li>
<li><a href="https://github.com/taskchampion/taskchampion/"
class="uri">https://github.com/taskchampion/taskchampion/</a></li>
</ul>
</details>
