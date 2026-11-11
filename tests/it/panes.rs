//! tmux-flash over all panes of the window (`@flash-multi-pane on`): one
//! popup covers the window. The first pane is the target pane, a second one
//! is on its right.

use std::fmt::Write;

use crate::harness::Servers;

const MULTI_PANE: &str = "set -g @flash-multi-pane on\n";
const LEFT: &str = "alpha beta alpine zulu\n";
const RIGHT: &str = "gamma omega alps\n";
/// Columns of the left pane: half of the 80 columns, less the border.
const LEFT_WIDTH: usize = 40;

#[test]
fn label_jumps_into_other_pane() {
    let servers = Servers::start_with(LEFT, MULTI_PANE);
    let left = servers.pane();
    let right = servers.split(RIGHT);
    let mut flash = servers.flash();

    // "om" is in the right pane only. e follows it: the first label is a,
    // on the e.
    servers.type_text("om");
    servers.wait_for("the label", |s| s.screen().contains("omaga"));
    let first = servers.screen().lines().next().unwrap().to_owned();
    servers.type_text("a");

    assert!(servers.wait_exit(&mut flash));
    let left_text = LEFT.trim_end();
    assert_eq!(first, format!("{left_text:<LEFT_WIDTH$}│gamma omaga alps"));
    assert_eq!(servers.pane(), right);
    assert_eq!(servers.copy_cursor_in(&right), "copy-mode 6 0");
    assert_eq!(servers.copy_cursor_in(&left), "");
}

#[test]
fn other_panes_off_by_default() {
    let servers = Servers::start(LEFT);
    let right = servers.split(RIGHT);
    let mut flash = servers.flash();

    servers.type_text("om");

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.copy_cursor_in(&right), "");
}

#[test]
fn flag_turns_multi_pane_on() {
    let servers = Servers::start(LEFT);
    let right = servers.split(RIGHT);
    let mut flash = servers.flash_with(&["--multi-pane", "on"]);

    // As with the option: the first label a is on the e after "om".
    servers.type_text("om");
    servers.wait_for("the label", |s| s.screen().contains("omaga"));
    servers.type_text("a");

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.pane(), right);
}

#[test]
fn flag_turns_multi_pane_off() {
    let servers = Servers::start_with(LEFT, MULTI_PANE);
    let right = servers.split(RIGHT);
    let mut flash = servers.flash_with(&["--multi-pane", "off"]);

    servers.type_text("om");

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.copy_cursor_in(&right), "");
}

#[test]
fn enter_stays_in_target_pane() {
    let servers = Servers::start_with(LEFT, MULTI_PANE);
    let left = servers.pane();
    let _right = servers.split(RIGHT);
    let mut flash = servers.flash();

    // Both panes have "alp"; the cursor is below the left text: wrap to its
    // first match.
    servers.type_text("alp");
    servers.wait_for("the prompt", |s| s.prompt_is("alp"));
    servers.press("Enter");

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.pane(), left);
    assert_eq!(servers.copy_cursor(), "copy-mode 0 0");
}

#[test]
fn enter_needs_target_match() {
    let servers = Servers::start_with(LEFT, MULTI_PANE);
    let left = servers.pane();
    let right = servers.split(RIGHT);
    let mut flash = servers.flash();

    servers.type_text("om");
    servers.wait_for("the prompt", |s| s.prompt_is("om"));
    servers.press("Enter");

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.pane(), left);
    assert_eq!(servers.copy_cursor_in(&left), "");
    assert_eq!(servers.copy_cursor_in(&right), "");
}

#[test]
fn zoomed_window_has_one_pane() {
    let servers = Servers::start_with(LEFT, MULTI_PANE);
    let left = servers.pane();
    let right = servers.split(RIGHT);
    let _ = servers.inner(&["resize-pane", "-Z", "-t", &left]);
    let mut flash = servers.flash();

    servers.type_text("om");

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.copy_cursor_in(&right), "");
}

#[test]
fn pane_in_other_mode_is_blank() {
    let servers = Servers::start_with(LEFT, MULTI_PANE);
    let right = servers.split(RIGHT);
    let _ = servers.inner(&["choose-tree", "-t", &right]);
    let mut flash = servers.flash();

    // The text under the tree has "om", but the pane shows the tree: the
    // popup shows the pane blank.
    let left_text = LEFT.trim_end();
    let blank = format!("{left_text:<LEFT_WIDTH$}│");
    servers.wait_for("the blank pane", |s| {
        s.screen().lines().next() == Some(blank.as_str())
    });
    servers.type_text("om");

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.copy_cursor_in(&right), "tree-mode");
}

#[test]
fn scrolled_other_pane_is_kept() {
    let mut text = String::new();
    for n in 0..60 {
        writeln!(text, "line{n:02} text").unwrap();
    }
    let servers = Servers::start_with(LEFT, MULTI_PANE);
    let right = servers.split(&text);
    let _ = servers.inner(&["copy-mode", "-t", &right]);
    let _ = servers.inner(&["send-keys", "-t", &right, "-X", "-N", "10", "scroll-up"]);
    let view = servers.inner(&["capture-pane", "-p", "-t", &right, "-S", "-10", "-E", "0"]);
    let third: String = view.lines().nth(2).unwrap().chars().take(6).collect();

    let mut flash = servers.flash();
    servers.type_text(&third);
    servers.wait_for("the prompt", |s| s.prompt_is(&third));

    // The label covers the space after the match.
    let screen = servers.screen();
    let row = screen.lines().nth(2).unwrap();
    let label = row.split(&third).nth(1).unwrap().chars().next().unwrap();
    servers.type_text(&label.to_string());

    assert!(servers.wait_exit(&mut flash));
    assert_eq!(servers.pane(), right);
    assert_eq!(servers.copy_cursor_in(&right), "copy-mode 0 2");
    let scroll = servers.inner(&["display-message", "-p", "-t", &right, "#{scroll_position}"]);
    assert_eq!(scroll.trim(), "10");
}
