---
title: Key Configuration
description: Customize taskwarrior-tui keybindings through your Taskwarrior configuration file.
---

Configure `taskwarrior-tui` using `~/.taskrc`:

`taskwarrior-tui` reads values from your Taskwarrior `taskrc` file (default: `~/.taskrc`).

```plaintext
uda.taskwarrior-tui.keyconfig.quit=q
# Refresh defaults to Ctrl+R.
uda.taskwarrior-tui.keyconfig.go-to-bottom=G
uda.taskwarrior-tui.keyconfig.go-to-top=g
uda.taskwarrior-tui.keyconfig.down=j
uda.taskwarrior-tui.keyconfig.up=k
uda.taskwarrior-tui.keyconfig.page-down=J
uda.taskwarrior-tui.keyconfig.page-up=K
uda.taskwarrior-tui.keyconfig.delete=x
uda.taskwarrior-tui.keyconfig.done=d
uda.taskwarrior-tui.keyconfig.start-stop=s
uda.taskwarrior-tui.keyconfig.quick-tag=t
uda.taskwarrior-tui.keyconfig.undo=u
uda.taskwarrior-tui.keyconfig.edit=e
uda.taskwarrior-tui.keyconfig.duplicate=y
uda.taskwarrior-tui.keyconfig.modify=m
uda.taskwarrior-tui.keyconfig.shell=!
uda.taskwarrior-tui.keyconfig.log=l
uda.taskwarrior-tui.keyconfig.add=a
uda.taskwarrior-tui.keyconfig.annotate=A
uda.taskwarrior-tui.keyconfig.filter=/
uda.taskwarrior-tui.keyconfig.zoom=z
uda.taskwarrior-tui.keyconfig.context-menu=c
uda.taskwarrior-tui.keyconfig.report-menu=r
uda.taskwarrior-tui.keyconfig.next-tab=]
uda.taskwarrior-tui.keyconfig.previous-tab=[
```

Press `r` to choose a report. Type to filter its name or effective filter, use the
arrow keys to move, and press Enter to load it. Press `1` to `9` to load that
numbered row and close the picker. Numbers follow the filtered list, and digits
`1` to `9` select rows rather than entering search text. The Active column marks
the current report. Press `Ctrl+R` to refresh the task list.
