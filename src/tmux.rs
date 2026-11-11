//! The tmux program. This is the only module that starts processes.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::process::Command;

use crate::flash::screen::Point;
use crate::flash::session::Target;

const TMUX: &str = "tmux";
/// Without a UTF-8 locale (cron, containers, a bare run-shell), tmux turns
/// tabs and non-ASCII text in its output into `_`; -u keeps them.
const UTF8: &str = "-u";
/// `pane_mode` of a pane in copy mode.
const COPY_MODE: &str = "copy-mode";
/// `pane_mode` of a pane in no mode.
const NO_MODE: &str = "";
/// Separates tmux commands in one call.
const SEPARATOR: &str = ";";
const FIELD: char = '\t';
/// tmux formats give flags as 1 and 0.
const FLAG_ON: &str = "1";
const PLACEMENT_FORMAT: &str = "#{pane_width}\t#{pane_height}\t#{window_width}\t#{window_height}\t\
                                #{window_panes}\t#{window_zoomed_flag}\t#{window_bigger}\t\
                                #{@flash-multi-pane}";
/// The user options of tmux-flash come with the state: no extra tmux call.
const STATE_FORMAT: &str = "#{pane_width}\t#{pane_height}\t#{pane_mode}\t#{scroll_position}\t\
                            #{cursor_x}\t#{cursor_y}\t#{copy_cursor_x}\t#{copy_cursor_y}\t\
                            #{@color-mode}\t#{@flash-labels}";
/// `list-panes` gives each pane its place and the window size before the
/// state.
const LISTED_FORMAT: &str =
    "#{pane_id}\t#{pane_left}\t#{pane_top}\t#{window_width}\t#{window_height}\t";
/// The fields of `LISTED_FORMAT`.
const LISTED_FIELDS: usize = 5;
/// The popup over the pane: -y is the bottom row of a popup.
const PANE_X: &str = "#{popup_pane_left}";
const PANE_Y: &str = "#{popup_pane_bottom}";
/// The popup over the window, from the place of the pane in it. The
/// `popup_pane_*` formats count the status line and the client offset.
const WINDOW_X: &str = "#{e|-:#{popup_pane_left},#{pane_left}}";
const WINDOW_Y: &str =
    "#{e|+:#{popup_pane_bottom},#{e|-:#{e|-:#{window_height},#{pane_top}},#{pane_height}}}";
/// `display-message -d`: 0 keeps the message until the next one, 1
/// removes it at once.
const UNTIL_CLEARED: &str = "0";
const AT_ONCE: &str = "1";

/// Width and height.
type Size = (usize, usize);

/// Whether the pane shows copy mode, no mode, or another mode (such as
/// `tree-mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneMode {
    Normal,
    Copy,
    Other,
}

/// Size, mode and cursor of a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaneState {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) mode: PaneMode,
    /// Rows the copy mode view is scrolled up.
    pub(crate) scroll: usize,
    pub(crate) cursor: Point,
}

/// The user options `@color-mode` and `@flash-labels`, empty when not set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) color_mode: String,
    pub(crate) labels: String,
}

/// What the launcher needs to select and place the popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) pane: Size,
    pub(crate) window: Size,
    /// Panes in the window.
    pub(crate) panes: usize,
    pub(crate) zoomed: bool,
    /// The window is larger than the client: tmux shows a part of it.
    pub(crate) bigger: bool,
    /// The user option `@flash-multi-pane`, empty when not set.
    pub(crate) multi_pane: String,
}

/// What the popup covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cover {
    Pane,
    Window,
}

/// Whether a jump makes its pane the active pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    Keep,
    Select,
}

/// A pane: its id, its place in the popup, its state and the text of its
/// visible rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneView {
    pub(crate) id: String,
    pub(crate) left: usize,
    pub(crate) top: usize,
    pub(crate) state: PaneState,
    pub(crate) text: String,
}

/// What the popup covers: its size, the options, the target pane and the
/// other panes in pane index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct View {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) options: Options,
    pub(crate) target: PaneView,
    pub(crate) others: Vec<PaneView>,
}

/// One line of `list-panes`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Listed {
    id: String,
    left: usize,
    top: usize,
    window: Size,
    state: PaneState,
    options: Options,
}

/// A text in the tmux message line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Message<'a> {
    /// Stays until it is cleared.
    Prompt(&'a str),
    Clear,
    /// Stays for the tmux `display-time`.
    Error(&'a str),
}

#[derive(Debug)]
pub(crate) enum TmuxError {
    Start(io::Error),
    Failed(String),
    Output(String),
}

impl fmt::Display for TmuxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(error) => write!(f, "cannot start tmux: {error}"),
            Self::Failed(stderr) => write!(f, "tmux failed: {}", stderr.trim()),
            Self::Output(text) => write!(f, "unexpected tmux output: {}", text.trim()),
        }
    }
}

/// The sizes of the pane and its window, and the window state.
pub(crate) fn placement(pane: &str) -> Result<Placement, TmuxError> {
    let text = run(&["display-message", "-p", "-t", pane, PLACEMENT_FORMAT])?;

    parse_placement(&text)
}

/// Runs `command` in a popup over the pane or its window, and waits until
/// it closes.
pub(crate) fn open_popup(
    client: &str,
    pane: &str,
    cover: Cover,
    size: Size,
    command: &str,
) -> Result<(), TmuxError> {
    let _ = run(&popup_args(client, pane, cover, size, command))?;

    Ok(())
}

/// The pane alone, at the top left of the popup.
pub(crate) fn pane_view(pane: &str) -> Result<View, TmuxError> {
    let (state, options) =
        parse_state(&run(&["display-message", "-p", "-t", pane, STATE_FORMAT])?)?;
    let text = run(&capture_args(pane, &state))?;

    Ok(View {
        width: state.width,
        height: state.height,
        options,
        target: PaneView {
            id: pane.to_owned(),
            left: 0,
            top: 0,
            state,
            text,
        },
        others: Vec::new(),
    })
}

/// All panes of the window of the pane, at their places in the window.
pub(crate) fn window_view(pane: &str) -> Result<View, TmuxError> {
    let format = format!("{LISTED_FORMAT}{STATE_FORMAT}");
    let listing = run(&["list-panes", "-t", pane, "-F", &format])?;
    let listed = listing
        .lines()
        .map(parse_listed)
        .collect::<Result<Vec<_>, _>>()?;

    let captured = run(&captures_args(&listed))?;
    let heights: Vec<usize> = listed.iter().map(|one| one.state.height).collect();
    let texts = split_rows(&captured, &heights);

    window_of(pane, listed, texts)
}

/// Puts the pane in copy mode with the cursor on the target.
pub(crate) fn jump(
    pane: &str,
    mode: PaneMode,
    target: Target,
    focus: Focus,
) -> Result<(), TmuxError> {
    let _ = run(&jump_args(pane, mode, target, focus))?;

    Ok(())
}

pub(crate) fn message(client: &str, message: &Message<'_>) -> Result<(), TmuxError> {
    let _ = run(&message_args(client, message))?;

    Ok(())
}

/// One shell word, for the command that the popup runs.
pub(crate) fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn run<S: AsRef<OsStr>>(args: &[S]) -> Result<String, TmuxError> {
    let output = Command::new(TMUX)
        .arg(UTF8)
        .args(args)
        .output()
        .map_err(TmuxError::Start)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(TmuxError::Failed(stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_placement(text: &str) -> Result<Placement, TmuxError> {
    let fields: Vec<&str> = text.trim_end_matches('\n').split(FIELD).collect();
    let [
        pane_width,
        pane_height,
        window_width,
        window_height,
        panes,
        zoomed,
        bigger,
        multi_pane,
    ] = fields.as_slice()
    else {
        return Err(TmuxError::Output(text.to_owned()));
    };

    Ok(Placement {
        pane: (number(pane_width, text)?, number(pane_height, text)?),
        window: (number(window_width, text)?, number(window_height, text)?),
        panes: number(panes, text)?,
        zoomed: *zoomed == FLAG_ON,
        bigger: *bigger == FLAG_ON,
        multi_pane: (*multi_pane).to_owned(),
    })
}

fn parse_state(text: &str) -> Result<(PaneState, Options), TmuxError> {
    let fields: Vec<&str> = text.trim_end_matches('\n').split(FIELD).collect();
    let [
        width,
        height,
        mode,
        scroll,
        x,
        y,
        copy_x,
        copy_y,
        color_mode,
        labels,
    ] = fields.as_slice()
    else {
        return Err(TmuxError::Output(text.to_owned()));
    };

    let mode = match *mode {
        NO_MODE => PaneMode::Normal,
        COPY_MODE => PaneMode::Copy,
        _ => PaneMode::Other,
    };
    let (x, y) = match mode {
        PaneMode::Normal | PaneMode::Other => (x, y),
        PaneMode::Copy => (copy_x, copy_y),
    };
    let scroll = if scroll.is_empty() {
        0
    } else {
        number(scroll, text)?
    };

    let state = PaneState {
        width: number(width, text)?,
        height: number(height, text)?,
        mode,
        scroll,
        cursor: Point {
            row: number(y, text)?,
            col: number(x, text)?,
        },
    };
    let options = Options {
        color_mode: (*color_mode).to_owned(),
        labels: (*labels).to_owned(),
    };

    Ok((state, options))
}

fn parse_listed(line: &str) -> Result<Listed, TmuxError> {
    let fields: Vec<&str> = line.splitn(LISTED_FIELDS + 1, FIELD).collect();
    let [id, left, top, width, height, state] = fields.as_slice() else {
        return Err(TmuxError::Output(line.to_owned()));
    };
    let (state, options) = parse_state(state)?;

    Ok(Listed {
        id: (*id).to_owned(),
        left: number(left, line)?,
        top: number(top, line)?,
        window: (number(width, line)?, number(height, line)?),
        state,
        options,
    })
}

/// The target pane first; the window size and the options from its line.
fn window_of(pane: &str, listed: Vec<Listed>, texts: Vec<String>) -> Result<View, TmuxError> {
    let mut panes: Vec<(Listed, String)> = listed.into_iter().zip(texts).collect();
    let Some(position) = panes.iter().position(|(one, _)| one.id == pane) else {
        return Err(TmuxError::Output(format!("no pane {pane} in its window")));
    };

    let (target, text) = panes.remove(position);
    let (width, height) = target.window;
    let options = target.options.clone();

    Ok(View {
        width,
        height,
        options,
        target: view_of(target, text),
        others: panes
            .into_iter()
            .map(|(one, text)| view_of(one, text))
            .collect(),
    })
}

fn view_of(listed: Listed, text: String) -> PaneView {
    PaneView {
        id: listed.id,
        left: listed.left,
        top: listed.top,
        state: listed.state,
        text,
    }
}

/// The text of each pane from one capture of all: `heights` rows each.
/// capture-pane prints each row of the range, empty ones too.
fn split_rows(text: &str, heights: &[usize]) -> Vec<String> {
    let mut rows = text.split_inclusive('\n');

    heights
        .iter()
        .map(|&height| rows.by_ref().take(height).collect())
        .collect()
}

fn number(field: &str, text: &str) -> Result<usize, TmuxError> {
    field
        .parse()
        .map_err(|_| TmuxError::Output(text.to_owned()))
}

fn capture_args(pane: &str, state: &PaneState) -> Vec<String> {
    let mut args = strings(["capture-pane", "-p", "-t", pane]);
    if state.mode != PaneMode::Copy || state.scroll == 0 {
        return args;
    }

    // Rows count from the bottom of the history: the view starts `scroll`
    // rows up.
    let last = state.height.saturating_sub(1);
    let end = if last >= state.scroll {
        (last - state.scroll).to_string()
    } else {
        format!("-{}", state.scroll - last)
    };

    args.extend([
        "-S".to_owned(),
        format!("-{}", state.scroll),
        "-E".to_owned(),
        end,
    ]);
    args
}

/// All panes in one tmux call.
fn captures_args(listed: &[Listed]) -> Vec<String> {
    let commands: Vec<Vec<String>> = listed
        .iter()
        .map(|one| capture_args(&one.id, &one.state))
        .collect();

    commands.join(&SEPARATOR.to_owned())
}

fn jump_args(pane: &str, mode: PaneMode, target: Target, focus: Focus) -> Vec<String> {
    let mut commands = Vec::new();

    if focus == Focus::Select {
        commands.push(strings(["select-pane", "-t", pane]));
    }

    if mode != PaneMode::Copy {
        commands.push(strings(["copy-mode", "-t", pane]));
    }

    commands.push(copy_command(pane, "top-line", None));
    if target.row > 0 {
        commands.push(copy_command(pane, "cursor-down", Some(target.row)));
    }

    commands.push(copy_command(pane, "start-of-line", None));
    if target.index > 0 {
        commands.push(copy_command(pane, "cursor-right", Some(target.index)));
    }

    commands.join(&SEPARATOR.to_owned())
}

fn copy_command(pane: &str, command: &str, repeat: Option<usize>) -> Vec<String> {
    let mut args = strings(["send-keys", "-t", pane, "-X"]);
    if let Some(count) = repeat {
        args.extend(["-N".to_owned(), count.to_string()]);
    }

    args.push(command.to_owned());
    args
}

fn popup_args(client: &str, pane: &str, cover: Cover, size: Size, command: &str) -> Vec<String> {
    let (width, height) = (size.0.to_string(), size.1.to_string());
    let (x, y) = match cover {
        Cover::Pane => (PANE_X, PANE_Y),
        Cover::Window => (WINDOW_X, WINDOW_Y),
    };

    // -x and -y take formats, -w and -h do not ("height invalid").
    strings([
        "display-popup",
        "-B",
        "-E",
        "-c",
        client,
        "-t",
        pane,
        "-x",
        x,
        "-y",
        y,
        "-w",
        &width,
        "-h",
        &height,
        command,
    ])
}

fn message_args(client: &str, message: &Message<'_>) -> Vec<String> {
    let mut args = strings(["display-message", "-c", client]);

    match message {
        Message::Prompt(text) => {
            args.extend(["-d".to_owned(), UNTIL_CLEARED.to_owned(), escape(text)]);
        }
        Message::Clear => args.extend(["-d".to_owned(), AT_ONCE.to_owned(), String::new()]),
        Message::Error(text) => args.push(escape(text)),
    }

    args
}

/// display-message expands formats; `##` is a literal `#`.
fn escape(text: &str) -> String {
    text.replace('#', "##")
}

fn strings<const N: usize>(args: [&str; N]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

#[cfg(test)]
#[path = "tmux_test.rs"]
mod test;
