//! The two runs of tmux-flash. The launcher opens a popup over the pane or
//! its window; the popup runs tmux-flash again for the session.

use std::fmt;
use std::io::{self, Write};
use std::iter;

use crate::flash::screen::{CURSOR_PANE, Screen};
use crate::flash::session::{Session, Step, Target};
use crate::paint::{Layout, Rect, paint};
use crate::term::Term;
use crate::theme::Theme;
use crate::tmux::{self, Cover, Focus, Message, PaneMode, PaneView, Placement, TmuxError, View};

pub(crate) const PANE_FLAG: &str = "--pane";
pub(crate) const CLIENT_FLAG: &str = "--client";
/// Runs the session; the launcher gives it to the popup.
pub(crate) const UI_FLAG: &str = "--ui";
/// The session covers all panes of the window; the launcher gives it to the
/// popup.
pub(crate) const WINDOW_FLAG: &str = "--window";

const NAME: &str = env!("CARGO_PKG_NAME");
/// The flash.nvim default labels, used when `@flash-labels` is not set.
const DEFAULT_LABELS: &str = "asdfghjklqwertyuiopzxcvbnm";
const PROMPT: &str = "⚡";
/// The values of `--multi-pane` and `@flash-multi-pane`.
const MULTI_PANE_ON: &str = "on";
const MULTI_PANE_OFF: &str = "off";

/// The pane to jump in and the client that pressed the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Origin {
    pub(crate) pane: String,
    pub(crate) client: String,
}

/// What the session covers: the pane, or all panes of its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Pane,
    Window,
}

/// Whether the launcher searches the other panes too: `--multi-pane`, else
/// `@flash-multi-pane`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MultiPane {
    FromOption,
    On,
    Off,
}

impl MultiPane {
    /// The flag and the option take the same values.
    pub(crate) fn from_value(value: &str) -> Option<Self> {
        match value.trim() {
            MULTI_PANE_ON => Some(Self::On),
            MULTI_PANE_OFF => Some(Self::Off),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum AppError {
    Tmux(TmuxError),
    Io(io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tmux(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl From<TmuxError> for AppError {
    fn from(error: TmuxError) -> Self {
        Self::Tmux(error)
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Opens the popup and waits until it closes.
pub(crate) fn launch(origin: &Origin, multi_pane: MultiPane) -> Result<(), AppError> {
    let placement = tmux::placement(&origin.pane)?;
    let scope = scope_of(&placement, multi_pane);
    let exe = std::env::current_exe()?;
    let command = ui_command(&exe.to_string_lossy(), origin, scope);
    let (cover, size) = match scope {
        Scope::Pane => (Cover::Pane, placement.pane),
        Scope::Window => (Cover::Window, placement.window),
    };

    tmux::open_popup(&origin.client, &origin.pane, cover, size, &command)?;
    Ok(())
}

/// The session in the popup: draw, read a key, until a jump or an exit.
pub(crate) fn run_ui(origin: &Origin, scope: Scope) -> Result<(), AppError> {
    let view = match scope {
        Scope::Pane => tmux::pane_view(&origin.pane)?,
        Scope::Window => tmux::window_view(&origin.pane)?,
    };
    // Without the option the colors are dark, as the tmux status line.
    let theme = Theme::from_option(&view.options.color_mode);
    let labels = labels_from(&view.options.labels);
    let layout = layout_of(&view);
    let mut session = Session::new(screens_of(&view), view.target.state.cursor, &labels);

    let prompt = Prompt {
        client: &origin.client,
    };
    let mut term = Term::open()?;

    loop {
        term.draw(&paint(&session, &layout), theme)?;
        prompt.show(session.pattern())?;

        match session.step(term.read_key()?) {
            Step::Continue => {}
            Step::Exit => return Ok(()),
            Step::Jump(target) => return jump(&view, target),
        }
    }
}

/// In the message line of the client; to stderr when tmux cannot show it.
pub(crate) fn report(origin: &Origin, error: &AppError) {
    let text = format!("{NAME}: {error}");
    if tmux::message(&origin.client, &Message::Error(&text)).is_ok() {
        return;
    }

    // If stderr is gone, there is no other place to report to.
    let _ = writeln!(io::stderr(), "{text}");
}

/// The pattern in the message line. A prompt stays after the popup closes,
/// so it is cleared on every way out.
struct Prompt<'a> {
    client: &'a str,
}

impl Prompt<'_> {
    fn show(&self, pattern: &str) -> Result<(), TmuxError> {
        tmux::message(
            self.client,
            &Message::Prompt(&format!("{PROMPT} {pattern}")),
        )
    }
}

impl Drop for Prompt<'_> {
    fn drop(&mut self) {
        let _ = tmux::message(self.client, &Message::Clear);
    }
}

/// The labels of `@flash-labels`; the default when it is not set. Spaces
/// and control characters cannot be typed as a label.
fn labels_from(option: &str) -> String {
    let labels: String = option
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    if labels.is_empty() {
        return DEFAULT_LABELS.to_owned();
    }

    labels
}

/// flash.nvim searches all windows; here other panes are searched only when
/// `--multi-pane` or `@flash-multi-pane` is on, by request. A zoomed window
/// shows one pane, and of a window larger than the client tmux shows a part
/// only.
fn scope_of(placement: &Placement, multi_pane: MultiPane) -> Scope {
    let multi_pane = match multi_pane {
        MultiPane::FromOption => MultiPane::from_value(&placement.multi_pane),
        given => Some(given),
    };
    let on = multi_pane == Some(MultiPane::On);
    if !on || placement.panes <= 1 || placement.zoomed || placement.bigger {
        return Scope::Pane;
    }

    Scope::Window
}

fn ui_command(exe: &str, origin: &Origin, scope: Scope) -> String {
    let window = match scope {
        Scope::Pane => String::new(),
        Scope::Window => format!(" {WINDOW_FLAG}"),
    };

    format!(
        "{} {UI_FLAG}{window} {PANE_FLAG} {} {CLIENT_FLAG} {}",
        tmux::quote(exe),
        tmux::quote(&origin.pane),
        tmux::quote(&origin.client),
    )
}

/// The panes in session order: the target pane first.
fn panes(view: &View) -> impl Iterator<Item = &PaneView> {
    iter::once(&view.target).chain(&view.others)
}

fn layout_of(view: &View) -> Layout {
    let rects = panes(view).map(|pane| Rect {
        left: pane.left,
        top: pane.top,
        width: pane.state.width,
        height: pane.state.height,
    });

    Layout {
        width: view.width,
        height: view.height,
        panes: rects.collect(),
    }
}

/// One screen per pane. Another pane in a mode other than copy mode shows no
/// text: capture-pane gives the text under the mode, which the pane does not
/// show. The target pane keeps its text, as when it is alone.
fn screens_of(view: &View) -> Vec<Screen> {
    let screen = |(index, pane): (usize, &PaneView)| {
        let state = pane.state;
        let blank = index != CURSOR_PANE && state.mode == PaneMode::Other;
        let text = if blank { "" } else { pane.text.as_str() };

        Screen::parse(text, state.width, state.height)
    };

    panes(view).enumerate().map(screen).collect()
}

fn jump(view: &View, target: Target) -> Result<(), AppError> {
    let Some(pane) = panes(view).nth(target.pane) else {
        return Ok(());
    };
    let focus = if target.pane == CURSOR_PANE {
        Focus::Keep
    } else {
        Focus::Select
    };

    tmux::jump(&pane.id, pane.state.mode, target, focus)?;
    Ok(())
}

#[cfg(test)]
#[path = "app_test.rs"]
mod test;
