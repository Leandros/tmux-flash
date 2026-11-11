use super::*;
use crate::flash::screen::Point;
use crate::tmux::{Options, PaneMode, PaneState, PaneView, Placement, View};

/// No `--multi-pane` flag: `@flash-multi-pane` decides.
const FROM_OPTION: MultiPane = MultiPane::FromOption;

#[test]
fn unset_option_gives_default() {
    assert_eq!(labels_from(""), DEFAULT_LABELS);
    assert_eq!(DEFAULT_LABELS, "asdfghjklqwertyuiopzxcvbnm");
}

#[test]
fn blank_option_gives_default() {
    assert_eq!(labels_from("  \t"), DEFAULT_LABELS);
}

#[test]
fn option_gives_the_labels() {
    assert_eq!(labels_from("ctie"), "ctie");
}

#[test]
fn spaces_are_no_labels() {
    assert_eq!(labels_from(" c t\n"), "ct");
}

fn placement(multi_pane: &str) -> Placement {
    Placement {
        pane: (39, 11),
        window: (80, 23),
        panes: 2,
        zoomed: false,
        bigger: false,
        multi_pane: multi_pane.to_owned(),
    }
}

#[test]
fn multi_pane_is_off_by_default() {
    assert_eq!(scope_of(&placement(""), FROM_OPTION), Scope::Pane);
    assert_eq!(scope_of(&placement("off"), FROM_OPTION), Scope::Pane);
}

#[test]
fn multi_pane_covers_the_window() {
    assert_eq!(scope_of(&placement("on"), FROM_OPTION), Scope::Window);
}

#[test]
fn flag_overrides_the_option() {
    assert_eq!(scope_of(&placement(""), MultiPane::On), Scope::Window);
    assert_eq!(scope_of(&placement("on"), MultiPane::Off), Scope::Pane);
}

#[test]
fn values_are_on_and_off() {
    assert_eq!(MultiPane::from_value("on"), Some(MultiPane::On));
    assert_eq!(MultiPane::from_value("off"), Some(MultiPane::Off));
    assert_eq!(MultiPane::from_value("yes"), None);
}

#[test]
fn one_pane_needs_no_window() {
    let one = Placement {
        panes: 1,
        ..placement("on")
    };

    assert_eq!(scope_of(&one, FROM_OPTION), Scope::Pane);
}

#[test]
fn zoomed_window_shows_one_pane() {
    let zoomed = Placement {
        zoomed: true,
        ..placement("on")
    };

    assert_eq!(scope_of(&zoomed, FROM_OPTION), Scope::Pane);
    assert_eq!(scope_of(&zoomed, MultiPane::On), Scope::Pane);
}

#[test]
fn larger_window_uses_one_pane() {
    let bigger = Placement {
        bigger: true,
        ..placement("on")
    };

    assert_eq!(scope_of(&bigger, FROM_OPTION), Scope::Pane);
}

#[test]
fn ui_command_has_the_scope() {
    let origin = Origin {
        pane: "%1".to_owned(),
        client: "/dev/pts/3".to_owned(),
    };

    assert_eq!(
        ui_command("/bin/tmux-flash", &origin, Scope::Pane),
        "'/bin/tmux-flash' --ui --pane '%1' --client '/dev/pts/3'"
    );
    assert_eq!(
        ui_command("/bin/tmux-flash", &origin, Scope::Window),
        "'/bin/tmux-flash' --ui --window --pane '%1' --client '/dev/pts/3'"
    );
}

fn pane(id: &str, left: usize, mode: PaneMode, text: &str) -> PaneView {
    PaneView {
        id: id.to_owned(),
        left,
        top: 0,
        state: PaneState {
            width: 3,
            height: 1,
            mode,
            scroll: 0,
            cursor: Point { row: 0, col: 0 },
        },
        text: text.to_owned(),
    }
}

fn view(target: PaneView, others: Vec<PaneView>) -> View {
    View {
        width: 7,
        height: 1,
        options: Options::default(),
        target,
        others,
    }
}

#[test]
fn layout_has_each_pane() {
    let window = view(
        pane("%2", 4, PaneMode::Normal, "b\n"),
        vec![pane("%1", 0, PaneMode::Normal, "a\n")],
    );

    assert_eq!(
        layout_of(&window),
        Layout {
            width: 7,
            height: 1,
            panes: vec![
                Rect {
                    left: 4,
                    top: 0,
                    width: 3,
                    height: 1,
                },
                Rect {
                    left: 0,
                    top: 0,
                    width: 3,
                    height: 1,
                },
            ],
        }
    );
}

#[test]
fn other_mode_pane_is_blank() {
    // capture-pane gives the text under the mode, not what the pane shows.
    let window = view(
        pane("%2", 4, PaneMode::Other, "b\n"),
        vec![pane("%1", 0, PaneMode::Other, "a\n")],
    );

    let screens = screens_of(&window);

    assert_eq!(screens[0], Screen::parse("b\n", 3, 1));
    assert_eq!(screens[1], Screen::parse("", 3, 1));
}
