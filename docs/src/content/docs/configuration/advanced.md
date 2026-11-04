---
title: Advanced Configuration
description: Configure taskwarrior-tui report behavior, styles, shortcuts, background tasks, and environment overrides.
---

`taskwarrior-tui` parses the output of `task show` to get configuration data. This allows `taskwarrior-tui` to use the same defaults as `taskwarrior` and configure additional options as required.

## `taskrc` Configuration File Options

Other `taskwarrior-tui` configuration options are possible using Taskwarrior user-defined attributes. All `taskwarrior-tui` specific configuration options begin with `uda.taskwarrior-tui.`. The following is the full list of options available and their default values if they are not defined in your `taskrc` file.

```plaintext
uda.taskwarrior-tui.selection.indicator=•
uda.taskwarrior-tui.selection.bold=yes
uda.taskwarrior-tui.selection.italic=no
uda.taskwarrior-tui.selection.dim=no
uda.taskwarrior-tui.selection.blink=no
uda.taskwarrior-tui.selection.reverse=no
uda.taskwarrior-tui.mark.indicator=✔
uda.taskwarrior-tui.unmark.indicator=
uda.taskwarrior-tui.mark-selection.indicator=⦿
uda.taskwarrior-tui.unmark-selection.indicator=⦾
uda.taskwarrior-tui.calendar.months-per-row=4
uda.taskwarrior-tui.task-report.info-show=true
uda.taskwarrior-tui.task-report.info-location=auto
uda.taskwarrior-tui.task-report.looping=true
uda.taskwarrior-tui.task-report.use-alternate-style=true
uda.taskwarrior-tui.task-report.jump-to-task-on-add=true
uda.taskwarrior-tui.task-report.jump-to-task-on-modify=true
uda.taskwarrior-tui.task-report.prompt-on-undo=false
uda.taskwarrior-tui.task-report.prompt-on-delete=false
uda.taskwarrior-tui.task-report.prompt-on-done=false
uda.taskwarrior-tui.style.report.selection=
uda.taskwarrior-tui.style.context.active=black on rgb444
uda.taskwarrior-tui.style.report-menu.active=black on rgb444
uda.taskwarrior-tui.style.calendar.title=black on rgb444
uda.taskwarrior-tui.style.report.scrollbar=black
uda.taskwarrior-tui.scrollbar.indicator=█
uda.taskwarrior-tui.style.report.scrollbar.area=white
uda.taskwarrior-tui.scrollbar.area=║
uda.taskwarrior-tui.task-report.next.filter=$(task show report.next.filter)
uda.taskwarrior-tui.task-report.auto-insert-double-quotes-on-add=true
uda.taskwarrior-tui.task-report.auto-insert-double-quotes-on-annotate=true
uda.taskwarrior-tui.task-report.auto-insert-double-quotes-on-log=true
uda.taskwarrior-tui.task-report.reset-filter-on-esc=true
uda.taskwarrior-tui.context-menu.select-on-move=false
uda.taskwarrior-tui.context-menu.close-on-select=true
uda.taskwarrior-tui.report-menu.select-on-move=false
uda.taskwarrior-tui.report-menu.close-on-select=true
# Put frequently used reports first in the report picker (lowest integer first).
uda.taskwarrior-tui.report.inbox.order=1
uda.taskwarrior-tui.report.handoffs.order=2
uda.taskwarrior-tui.report.current.order=3
uda.taskwarrior-tui.tabs.change-focus-rotate=false
uda.taskwarrior-tui.quick-tag.name=next
# UI chrome styles (support all Taskwarrior color formats)
uda.taskwarrior-tui.style.title=         # default: LightCyan foreground
uda.taskwarrior-tui.style.title.border=  # default: White foreground
uda.taskwarrior-tui.style.help.gauge=    # default: Gray foreground
uda.taskwarrior-tui.style.command.error= # default: Red foreground
```

See [color configuration](./colors.md) for supported color formats and additional TUI style keys such as `uda.taskwarrior-tui.style.navbar` and `uda.taskwarrior-tui.style.command`.

Set `uda.taskwarrior-tui.report.<name>.order=<integer>` to place a report near the
top of the picker. Lower numbers come first. Reports with the same number sort
alphabetically, followed by reports with no order, also alphabetically. Unknown
report names and invalid values are ignored. Without order settings, all reports
sort alphabetically. Row numbers follow this order and restart at 1 when filtered.

The `uda.taskwarrior-tui.task-report.next.filter` variable defines the default view at program startup. Set this to any preconfigured report from `task reports`, or create your own report in Taskwarrior and specify its name here.

## Legacy Config Aliases

The following older keys are still accepted for backward compatibility, but they are legacy, should be migrated to the newer names, and will be removed in a future release:

```plaintext
uda.taskwarrior-tui.task-report.show-info      -> uda.taskwarrior-tui.task-report.info-show
uda.taskwarrior-tui.tasklist.vertical          -> uda.taskwarrior-tui.task-report.info-location
```

For the legacy `tasklist.vertical` option, `true` maps to `bottom` and `false` maps to `right`. If both a legacy key and its newer replacement are set, the newer key takes precedence.

## Command-Line Options

`-r` specifies a report to be shown and overrides `uda.taskwarrior-tui.task-report.next.filter` for that instance.

## Configure Quick Tag

The quick-tag action toggles a single tag on the selected task. By default it uses the tag `next`, and the default keybinding is `t`.

Configure the tag name in your `taskrc` with a bare tag value:

```plaintext
uda.taskwarrior-tui.quick-tag.name=next
```

Do not include a leading `+`. `taskwarrior-tui` adds and removes the tag for you, so `next` becomes `+next` or `-next` when you trigger the quick-tag action.

## Cap Report Column Widths

When a report is wider than the screen, its widest columns shrink together, so long project names or tag lists take room from the description. Cap a column to stop it growing:

```plaintext
uda.taskwarrior-tui.task-report.column-max-width.project=20
uda.taskwarrior-tui.task-report.column-max-width.tags=24
```

Name the column as it appears in `report.<name>.columns`, without its format: `project` caps `project.parent` too. Longer values are clipped, and the description takes the room the cap frees. No column is capped by default.

## Configure User-Defined Shortcuts

You can configure shortcuts from your Taskwarrior `taskrc` file (default: `~/.taskrc`) by mapping them to executable files:

```plaintext
uda.taskwarrior-tui.shortcuts.1=~/.config/taskwarrior-tui/shortcut-scripts/add-personal-tag.sh
uda.taskwarrior-tui.shortcuts.2=~/.config/taskwarrior-tui/shortcut-scripts/sync.sh
...
```

The executable file can be placed in any location.

To make a file executable:

1. Run `chmod +x /path/to/script` to modify the executable flag.
2. Add `#!/usr/bin/env bash`, `#!/usr/bin/env python`, or whatever is appropriate for your script.

By default, keys `0`-`9` are available to run shortcuts.

When you hit the shortcut, the script will be executed with `selected_tasks_uuid` as an argument:

```bash
~/.config/taskwarrior-tui/shortcut-scripts/add-personal-tag.sh $selected_tasks_uuid
```

For example, you can add the `personal` tag to the currently selected task with the following script in `~/.config/taskwarrior-tui/shortcut-scripts/add-personal-tag.sh`:

```plaintext
task rc.bulk=0 rc.confirmation=off rc.dependency.confirmation=off rc.recurrence.confirmation=off "$@" modify +personal
```

A shortcut normally takes over the terminal until its script exits. A script that only changes tasks and needs no input can run in the background instead, queued behind the TUI's own writes, so the next key is handled at once:

```plaintext
uda.taskwarrior-tui.shortcuts.background=1,5
```

A background shortcut gets no terminal input, and its output shows only when it fails.

Every shortcut receives the active report name in `TASKWARRIOR_TUI_REPORT`, including background shortcuts. A script can use this variable to give the same key a different action in each report. Selected task UUIDs are still passed as arguments.

Configure a one-line key legend for each report:

```ini
uda.taskwarrior-tui.legend.default=1 open | 2 views | ^P project
uda.taskwarrior-tui.legend.handoffs=1 open | 2 views | 3 brief | 4 note | 5 promote | 6 reply | 7 ack
```

The active report name and legend share the heading above the filter, using the existing row. In tmux, the pane title also follows the active report. It changes when you select a report. A report without its own legend uses `legend.default`; an empty report legend hides it. Leave these settings unset to hide all legends. Long legends are clipped to the pane width.

A foreground shortcut can switch reports by writing a configured report name to `$TASKWARRIOR_TUI_REPORT_REQUEST` and exiting successfully:

```sh
printf '%s\n' handoffs > "$TASKWARRIOR_TUI_REPORT_REQUEST"
```

The TUI switches in place when the shortcut returns. An unknown report produces an error. The request file is temporary and is removed after each invocation; background shortcuts cannot request a report switch.

Reports can override the read context and detail pane without changing Taskwarrior's global context:

```ini
uda.taskwarrior-tui.report.handoffs.context=none
uda.taskwarrior-tui.report.handoffs.show-info=false
```

Other reports use the normal context and `task-report.show-info` setting. Task writes still use the selected UUIDs, and do not change the global context.

By default, shortcuts are linked to the `0-9` number row keys. They can be customized as any other keys through `uda.taskwarrior-tui.keyconfig.shortcut1=<key>`. For example:

```plaintext
uda.taskwarrior-tui.keyconfig.shortcut1=n
```

You can set up shortcuts to run `task sync` or any custom script that you like.

## Configure One Background Task

You can configure one background task to run periodically:

```plaintext
uda.taskwarrior-tui.background_process=task sync
uda.taskwarrior-tui.background_process_period=60
```

This runs `task sync` every 60 seconds. If `background_process` is an empty string, which is the default, then no process is run. Only if `background_process` is defined and runs successfully will it continue to run every `background_process_period` seconds, which defaults to 60. If it fails even once, it will not be run again until `taskwarrior-tui` is restarted.

## Environment Variables

### `TASKWARRIOR_TUI_DATA`

Overrides the `taskwarrior-tui` data directory used for logs and history files.

By default this uses your platform's local data directory, typically `~/.local/share/taskwarrior-tui/` on Linux.

This is useful when debugging because it lets you collect `taskwarrior-tui.log` in a temporary or project-local folder:

```bash
TASKWARRIOR_TUI_DATA=/tmp/taskwarrior-tui-debug taskwarrior-tui
```

### `TASKWARRIOR_TUI_LOG_LEVEL`

Controls how much information is written to `taskwarrior-tui.log`.

Supported values are `off`, `warn`, `info`, `debug`, and `trace`.

Examples:

```bash
TASKWARRIOR_TUI_LOG_LEVEL=debug taskwarrior-tui
TASKWARRIOR_TUI_LOG_LEVEL=trace taskwarrior-tui
```

### `TASKWARRIOR_TUI_TASKWARRIOR_CLI`

Overrides the path to the Taskwarrior executable. Defaults to `task`, resolved via `PATH`.

Use this if Taskwarrior is installed in a non-standard location, or if you want to wrap the `task` binary with a script:

```bash
TASKWARRIOR_TUI_TASKWARRIOR_CLI=/usr/local/bin/task taskwarrior-tui
```

You can also set it permanently in your shell profile:

```bash
export TASKWARRIOR_TUI_TASKWARRIOR_CLI=/usr/local/bin/task
```

In the normal task report, `Ctrl+P` adds the selected task's project to the visible filter. Press it again to restore the previous filter. Changing reports clears this temporary project scope. In input prompts, `Ctrl+P` keeps its completion-navigation behavior.

## Send a task to Gordon

Set a command in your Taskwarrior configuration:

```plaintext
uda.taskwarrior-tui.gordon.command=~/.local/bin/send-task-to-gordon
```

Press `Ctrl+G` to send the selected task without pressing Enter. The command runs in the background without input, with the selected task's full UUID appended as one argument. The command defaults to empty.
