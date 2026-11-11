# tmux-flash: context for changes

Read before changes in this repository
(<https://github.com/Leandros/tmux-flash>). The `AGENTS.md` rules of the user
apply; this file does not repeat them. The user commits. The user's dotfiles
install the release archives (`TMUX_FLASH_VERSION` in their `bootstrap.sh`).

## Purpose

Jump labels for a tmux pane with the rules of flash.nvim. It replaces the
tmux-jump plugin (Ruby, one character, word starts only).

## flash.nvim rules (source: `labeler.lua`, `state.lua`, `matcher.lua`)

- Search area: the visible rows of the target pane; in a scrolled copy mode
  the scrolled view. Exact substring match, anywhere in a line, overlapping
  matches, no match across lines.
- Case: smart case (lowercase pattern ignores case), as the user's nvim
  (`ignorecase` + `smartcase`).
- Label pool: the tmux option `@flash-labels` (whitespace and control
  characters dropped), else the flash.nvim default
  `asdfghjklqwertyuiopzxcvbnm`; then the same letters in uppercase;
  duplicates removed. The user's dotfiles set Bone 2 labels.
- Empty pattern: no labels.
- Skip rule: no label is a grapheme that directly follows a visible match.
  With smart or ignore case both cases are skipped (flash uses `ignorecase`
  there, not `smartcase`).
- Order: distance `|row*width+col - (cursor_row*width+cursor_col)|`, then
  reading order. Labels go in pool order.
- Reuse: a lowercase label stays on its position (row, grapheme index) while
  that position matches. Uppercase labels are not kept.
- Pool exhausted: the remaining matches are highlighted without a label.
- Current match: the first match strictly after the cursor, else the first
  match (wrap). It has a label too.
- Keys: a label jumps; another character extends the pattern; Backspace
  removes the last one; Enter jumps to the current match; Escape and Ctrl+C
  cancel. A character that leaves no match ends without a jump.
- Jump target: the first grapheme of the match.
- Other panes (flash `multi_window`), only with `--multi-pane on`, or
  without the flag with `@flash-multi-pane on` (off by default, by request;
  flash turns it on by default). The flag lets two key bindings search one
  pane and all panes:
  - The session has one screen per pane: the target pane first
    (`CURSOR_PANE`), then the others in pane index order.
  - Order: the target pane by distance; the other panes in reading order,
    one pane after the other (flash's labeler sorts the current window
    first, then the other windows by window id; distance counts in the
    current window only).
  - Skip rule and reuse span all panes. A reuse position is pane, row and
    grapheme index.
  - Current match: in the target pane only (flash `M:find` uses the matcher
    of the current window). Without one, Enter ends without a jump.
  - A character ends the session only when no pane has a match.
- Not done: regex and fuzzy modes, max_length, autojump, labels before the
  match.

## Design

- Two processes. The tmux binding runs `tmux-flash --pane P --client C` with
  `run-shell -b`. It reads the pane size and opens
  `display-popup -B -E -x '#{popup_pane_left}' -y '#{popup_pane_bottom}'
  -w W -h H`, which runs `tmux-flash --ui ...`. `-w` and `-h` do not take
  formats ("height invalid"). The popup covers the pane exactly (tested:
  split panes, zoomed pane, status and pane-border status at the top).
- Window scope: the launcher reads the pane and window sizes,
  `window_panes`, `window_zoomed_flag`, `window_bigger` and
  `@flash-multi-pane` in one `display-message` (`Placement`), and covers the
  window only with multi-pane on (`MultiPane`: `--multi-pane on|off`, else
  the option), more than one pane, not zoomed, and not bigger than the
  client (a part of it shows; the popup failed with "height too large"). It
  gives the popup `--ui --window`; `--window` without `--ui`, and
  `--multi-pane` with `--ui`, are usage errors. Popup position from the
  pane:
  `-x '#{e|-:#{popup_pane_left},#{pane_left}}'`,
  `-y '#{e|+:#{popup_pane_bottom},#{e|-:#{e|-:#{window_height},#{pane_top}},#{pane_height}}}'`.
  `popup_pane_top` is no pane top: with the status line at the top a popup
  from it was one row too high. Tested exact: status at the top and bottom,
  two status lines, pane-border status at the top and bottom, window smaller
  than the client.
- Window view: `list-panes -t P -F` with id, left, top, window size, then
  the state format; then one tmux call with a `capture-pane` per pane
  (`;`). capture-pane prints each row of the range, empty ones too, so the
  output splits by pane heights.
- `PaneMode::Other` (`tree-mode`, `view-mode`, ...): tmux 3.5a capture-pane
  reads the screen under the mode (no `-M`). Other panes in such a mode are
  blank, with no labels. The target pane keeps its text and gets copy mode
  on a jump, as when it is alone.
- Jump into another pane: `select-pane -t P` first, in the same tmux call.
- The frame (`paint`, `Layout`): each pane row at its offset, spaces fill a
  pane row before a border (not after the last pane: the erase does that),
  border cells are box-drawing lines that join the border cells next to
  them, in the backdrop color. Pane-border status lines show as lines.
  Widths by `unicode-width`, the rule of `Screen`: a cluster that tmux
  measures otherwise moves the rest of its row.
- Panes do not redraw while a popup is open. The popup gets raw keys: Enter
  `0d`, Backspace `7f`, Escape `1b` alone, Ctrl+C `03`.
- Jump: one tmux call: `copy-mode` (normal mode only), `top-line`,
  `cursor-down N`, `start-of-line`, `cursor-right K`. K is the number of
  grapheme clusters before the target in the captured line. On tmux 3.5a this
  was exact for ASCII, CJK, emoji, combining marks, ZWJ sequences, flags and
  tabs (`capture-pane` turns tabs into spaces). A tmux that does not join ZWJ
  sequences puts labels and the jump off after such an emoji.
- Capture: `capture-pane -p -t P`; in copy mode with scroll position S and
  height H: `-S -S -E H-1-S`. The cursor: `cursor_x/y`, in copy mode
  `copy_cursor_x/y`.
- Prompt `⚡ <pattern>`: `display-message -c C -d 0` (the tmux message line,
  as flash uses the command line). A `-d 0` message stays after the popup
  closes: every exit clears it with `display-message -c C -d 1 ""`.
- Colors: the user's nvim flash highlights (solarized.nvim autumn). Theme from
  the tmux option `@color-mode` (`dark` default, `light`):
  - dark: backdrop fg `#586e75`; match fg `#002b36` bg `#b58900`; current the
    same, bold; label fg `#002731` bg `#ffffff` bold.
  - light: backdrop fg `#93a1a1`; match fg `#fdf6e3` bg `#b58900`; current
    the same, bold; label fg `#fdf6e3` bg `#d33682` bold.
  The backdrop keeps the terminal background.
- Time to popup: about 23 ms (tmux 3.5a, each tmux call about 5 ms). The
  window scope makes as many tmux calls: 26 ms with three panes (debug
  build).
- Frames: `term` writes only the rows that changed, each ended with an
  erase to the line end. A clear of the whole popup before each frame made
  tmux show a blank popup between pty reads (a frame is about 30 kB with
  truecolor): fast typing flickered (blank popup rows in 0 to 6 of 300
  screen samples, 0 after the change). A `display-message` with a popup open
  redraws the popup region (about 6 kB); `tmux -u` makes no difference.
- Needs tmux 3.3: `display-popup -B` is new in 3.3 (`-x`/`-y` formats and
  `popup_pane_*` in 3.2). Tested on 3.4 (Ubuntu 24.04) and 3.5a.
- Every tmux call has `-u`: without a UTF-8 locale (containers, CI, a bare
  `run-shell`) tmux writes tabs and non-ASCII text of its output as `_`, and
  the tab-separated state did not parse ("unexpected tmux output: 80_23").
- Options come in the state format (`#{@color-mode}`, `#{@flash-labels}`),
  `@flash-multi-pane` in the placement format: no extra tmux call.

## Layers

`main` (arguments) → `app` (launcher, UI loop) → the pure modules `flash`
(rules: `screen`, `matcher`, `labeler`, `session`), `paint` (the frame as
styled spans) and `theme` (colors), and the adapters `tmux` (the only module
that starts processes) and `term` (the only module that reads keys or writes
escape sequences). The pure modules do no I/O.

- Smart case only: `Ignore` and `Sensitive` modes were dead code and went.
- `pub(crate)` everywhere (`unreachable_pub`); clippy's `redundant_pub_crate`
  asks for the opposite and is allowed in `Cargo.toml`.
- `display-message` expands formats: `#` in a message is written `##`.
- tmux output: the fields of `display-message -p` are tab separated
  (`pane_mode` and the copy cursor are empty outside copy mode).

## Tests

- Unit tests in `<file>_test.rs`; integration tests are modules of
  `tests/it/main.rs`.
- tmux tests use private servers: `TMUX` unset, own `TMUX_TMPDIR`. Never the
  caller's server. A second tmux server runs `tmux attach` in its pane as the
  client and gets the keys (`send-keys`); `capture-pane` of that pane shows
  what the client shows, popups included. Popups and `command-prompt` need a
  client.
- Wait with polling and a timeout, not fixed sleeps.
- `tests/it/harness.rs`: the inner server has `escape-time 10` (the 500 ms
  default delays Escape), the outer one `status off`. Wait for the `⚡`
  prompt before typing; `prompt_is` compares the whole message line.
- Expected labels follow reuse: they come from the first character typed
  ("a"), and stay when the pattern grows ("al"). Compute them that way.
- `Servers::start_with(text, conf)` adds inner config lines (an option).
  `Servers::split(text)` adds a pane on the right with `split-window -d`
  (the first pane stays active); `copy_cursor_in(pane)` reads any pane;
  `flash_with(args)` starts tmux-flash with more arguments.
  Multi-pane tests are in `tests/it/panes.rs`.
  tmux-flash runs with `LC_ALL=C`, so the tests catch locale problems.
  Socket paths stay short (`tf-<pid>-<n>`): macOS allows 104 bytes and its
  temp directory is long.
- CI: `.github/workflows/ci.yml`: fmt and clippy on `ubuntu-24.04`; build
  and test on `ubuntu-24.04`, `ubuntu-24.04-arm` and `macos-15` (no Intel
  macOS, by request). tmux from apt or brew; rustup when missing.
  `release.yml` calls it (`workflow_call`). Actions are pinned by commit.
  Check with
  `docker run --rm -v "$PWD:/repo:ro,z" -w /repo rhysd/actionlint:latest .github/workflows/ci.yml .github/workflows/release.yml`.
- `cargo clippy --locked --all-targets -- -D warnings` and
  `cargo fmt --check` must be clean.

## Release

- `release.yml`, `workflow_dispatch` on `master`. The user bumps `version`
  (and `Cargo.lock`) in a commit first; the workflow stops when
  `v<version>` exists (`git ls-remote`), and runs `cargo publish --dry-run`
  before anything is tagged.
- Archives `tmux-flash-<target>.tar.gz`, the binary at the top with README
  and licenses: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`
  (static: they run on AL2's glibc 2.26), `aarch64-apple-darwin`. The musl
  targets need no musl-tools (pure Rust; gcc links them).
- `gh release create v<version> --target $GITHUB_SHA --generate-notes` makes
  the tag on the tested commit; `SHA256SUMS` and build provenance
  attestations; then `cargo publish` with the `CARGO_REGISTRY_TOKEN` secret.
  No minisign signing, by request.
- cargo-binstall: `[package.metadata.binstall]` with the explicit `.tar.gz`
  name. binstall looks the crate up on crates.io and takes `repository`
  from it, so it works only after the publish. On a gnu Linux host it tries
  the gnu target, then musl (seen in its debug log).
- License: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`). `include`
  in `Cargo.toml` keeps `.kiro/`, `.github/` and `CLAUDE.md` out of the
  crate.
- Checked before the first release: the x86_64 musl build job in an
  `ubuntu:24.04` container (gcc links it; the binary runs on Amazon Linux 2),
  `cargo publish --dry-run`, and binstall's URL choice (gnu, then musl). A
  local plain-http server did not work for binstall: reqwest refused it
  (`Builder` error); test binstall against the real release.
