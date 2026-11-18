# Lumo — early alpha

Thanks for trying Lumo! It's a new file explorer for Windows, built to be fast,
private and pleasant to use. This is an **early alpha**: it works, but you are
among the first people outside the developer to use it, so expect rough edges.
Your feedback decides what gets fixed and built next.

## Getting started

1. Download **`Lumo-<version>.exe`** (or the `.zip`, which also contains the
   license files and this note). There's nothing to install: just run it.
2. Windows will probably say **"Windows protected your PC"**. That's because the
   alpha isn't code-signed yet (a paid certificate). Click **More info → Run
   anyway**. If you'd rather check the file first, compare its SHA-256 with
   `SHA256SUMS.txt` from the same release.
3. Lumo doesn't change any Windows settings and doesn't replace Explorer: both
   work side by side.

Requires Windows 10 (1809 or newer) or Windows 11, on a PC with a normal
graphics driver. Virtual machines or Remote Desktop without graphics support
may show "Lumo couldn't start"; if that happens, please tell us.

## Please be careful

Lumo copies, moves, renames and deletes **real files**. Deleted files go to
the Recycle Bin, nothing is ever overwritten, and almost everything can be
undone (Ctrl+Z) — but this is alpha software. **Try it on everyday folders,
not on irreplaceable files without a backup.**

## Things to try

- **Ctrl+K** — the command palette: type to find any command, tab, place or
  file in the current folder. Start with `>` for commands only.
- **Right-click** anything (or empty space) — a menu you can type into.
- Select several files (Ctrl/Shift+click, or the circles that appear on hover)
  — a tray with actions slides up.
- Copy/cut/paste files, also **to and from Explorer**.
- **F2** to rename, **Ctrl+Shift+N** for a new folder, **Delete** then the
  toast's **Undo**.
- **Recycle Bin** in the sidebar — restore or permanently delete.
- Tabs (**Ctrl+T**, **Ctrl+W**, **Ctrl+Tab**); close Lumo and reopen it — your
  tabs come back.
- Light and dark mode follow Windows; try both (Ctrl+K → "theme").

Not built yet: Win+E / opening folders from other apps in Lumo, search inside
subfolders, previews, drag and drop, Git integration, settings. They're coming.

## Telling us what you think

Anything is useful: bugs, confusing bits, things you loved or hated, what you
missed from Explorer.

- In Lumo: **Ctrl+K → "Report a problem"** opens a GitHub issue already filled
  in with your Lumo and Windows versions (you need a free GitHub account), or
- just message whoever sent you Lumo.

**If Lumo crashes**, it shows where it saved a crash report
(`%APPDATA%\Lumo\data\crash-….txt`). Please send that file — it tells us
exactly what went wrong. Lumo never sends anything over the internet by itself.

## Removing Lumo

Delete the `.exe`. To also remove its saved tabs and window position, delete
`%APPDATA%\Lumo`.
