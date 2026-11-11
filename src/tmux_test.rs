use super::*;

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

const TOP_LEFT: Target = Target {
    pane: 0,
    row: 0,
    index: 0,
};

#[test]
fn normal_mode_uses_cursor() {
    let (state, _) = parse_state("80\t24\t\t\t3\t5\t\t\t\t\n").unwrap();

    assert_eq!(
        state,
        PaneState {
            width: 80,
            height: 24,
            mode: PaneMode::Normal,
            scroll: 0,
            cursor: Point { row: 5, col: 3 },
        }
    );
}

#[test]
fn copy_mode_uses_copy_cursor() {
    let (state, _) = parse_state("80\t24\tcopy-mode\t7\t3\t5\t10\t2\t\t\n").unwrap();

    assert_eq!(state.mode, PaneMode::Copy);
    assert_eq!(state.scroll, 7);
    assert_eq!(state.cursor, Point { row: 2, col: 10 });
}

#[test]
fn other_modes_are_other() {
    let (state, _) = parse_state("80\t24\ttree-mode\t\t3\t5\t\t\t\t\n").unwrap();

    assert_eq!(state.mode, PaneMode::Other);
    assert_eq!(state.cursor, Point { row: 5, col: 3 });
}

#[test]
fn other_mode_acts_as_normal() {
    let (state, _) = parse_state("80\t9\ttree-mode\t7\t0\t0\t\t\t\t\n").unwrap();

    assert_eq!(
        capture_args("%1", &state),
        strings(&["capture-pane", "-p", "-t", "%1"])
    );
    assert_eq!(
        jump_args("%1", PaneMode::Other, TOP_LEFT, Focus::Keep)[..3],
        strings(&["copy-mode", "-t", "%1"])
    );
}

#[test]
fn options_follow_the_state() {
    let (_, options) = parse_state("80\t24\t\t\t3\t5\t\t\tlight\tabc\n").unwrap();

    assert_eq!(
        options,
        Options {
            color_mode: "light".to_owned(),
            labels: "abc".to_owned(),
        }
    );
}

#[test]
fn unset_options_are_empty() {
    let (_, options) = parse_state("80\t24\t\t\t3\t5\t\t\t\t\n").unwrap();

    assert_eq!(options, Options::default());
}

#[test]
fn bad_state_is_an_error() {
    assert!(parse_state("80\n").is_err());
    assert!(parse_state("80\t24\t\t\t3\t5\t\t\n").is_err());
    assert!(parse_state("x\t24\t\t\t3\t5\t\t\t\t\n").is_err());
}

#[test]
fn placement_fields() {
    assert_eq!(
        parse_placement("39\t11\t80\t23\t3\t0\t0\ton\n").unwrap(),
        Placement {
            pane: (39, 11),
            window: (80, 23),
            panes: 3,
            zoomed: false,
            bigger: false,
            multi_pane: "on".to_owned(),
        }
    );
}

#[test]
fn placement_flags() {
    let placement = parse_placement("80\t23\t80\t23\t2\t1\t1\t\n").unwrap();

    assert!(placement.zoomed);
    assert!(placement.bigger);
    assert_eq!(placement.multi_pane, "");
}

#[test]
fn bad_placement_is_an_error() {
    assert!(parse_placement("80\t24\n").is_err());
    assert!(parse_placement("80\t23\tx\t23\t2\t0\t0\t\n").is_err());
}

#[test]
fn capture_normal_mode() {
    let (state, _) = parse_state("80\t9\t\t\t0\t0\t\t\t\t\n").unwrap();

    assert_eq!(
        capture_args("%1", &state),
        strings(&["capture-pane", "-p", "-t", "%1"])
    );
}

#[test]
fn capture_scrolled_view() {
    let (near, _) = parse_state("80\t9\tcopy-mode\t7\t0\t0\t0\t0\t\t\n").unwrap();
    let (far, _) = parse_state("80\t9\tcopy-mode\t20\t0\t0\t0\t0\t\t\n").unwrap();

    assert_eq!(
        capture_args("%1", &near),
        strings(&["capture-pane", "-p", "-t", "%1", "-S", "-7", "-E", "1"])
    );
    assert_eq!(
        capture_args("%1", &far),
        strings(&["capture-pane", "-p", "-t", "%1", "-S", "-20", "-E", "-12"])
    );
}

#[test]
fn listed_pane_fields() {
    let listed =
        parse_listed("%3\t41\t12\t80\t23\t39\t11\tcopy-mode\t2\t0\t5\t4\t1\tlight\tab").unwrap();

    assert_eq!(
        listed,
        Listed {
            id: "%3".to_owned(),
            left: 41,
            top: 12,
            window: (80, 23),
            state: PaneState {
                width: 39,
                height: 11,
                mode: PaneMode::Copy,
                scroll: 2,
                cursor: Point { row: 1, col: 4 },
            },
            options: Options {
                color_mode: "light".to_owned(),
                labels: "ab".to_owned(),
            },
        }
    );
}

#[test]
fn bad_listed_pane_is_an_error() {
    assert!(parse_listed("%3\t41\t12").is_err());
    assert!(parse_listed("%3\tx\t12\t80\t23\t39\t11\t\t\t0\t5\t\t\t\t").is_err());
}

fn listed(id: &str, mode: &str) -> Listed {
    parse_listed(&format!(
        "{id}\t0\t0\t80\t23\t40\t9\t{mode}\t7\t0\t0\t0\t0\t\t"
    ))
    .unwrap()
}

#[test]
fn captures_in_one_call() {
    assert_eq!(
        captures_args(&[listed("%1", ""), listed("%2", "copy-mode")]),
        strings(&[
            "capture-pane",
            "-p",
            "-t",
            "%1",
            ";", //
            "capture-pane",
            "-p",
            "-t",
            "%2",
            "-S",
            "-7",
            "-E",
            "1",
        ])
    );
}

#[test]
fn rows_split_by_height() {
    assert_eq!(split_rows("a\nb\nc\n", &[2, 1, 1]), ["a\nb\n", "c\n", ""]);
}

#[test]
fn target_comes_first() {
    let all = vec![listed("%1", ""), listed("%2", ""), listed("%3", "")];
    let texts = strings(&["one\n", "two\n", "three\n"]);

    let view = window_of("%2", all, texts).unwrap();

    assert_eq!((view.width, view.height), (80, 23));
    assert_eq!(
        (view.target.id.as_str(), view.target.text.as_str()),
        ("%2", "two\n")
    );
    let others: Vec<&str> = view.others.iter().map(|pane| pane.id.as_str()).collect();
    assert_eq!(others, ["%1", "%3"]);
}

#[test]
fn missing_target_is_an_error() {
    let all = vec![listed("%1", "")];

    assert!(window_of("%2", all, strings(&["one\n"])).is_err());
}

#[test]
fn jump_starts_copy_mode() {
    let target = Target {
        pane: 0,
        row: 2,
        index: 3,
    };

    assert_eq!(
        jump_args("%1", PaneMode::Normal, target, Focus::Keep),
        strings(&[
            "copy-mode",
            "-t",
            "%1",
            ";", //
            "send-keys",
            "-t",
            "%1",
            "-X",
            "top-line",
            ";", //
            "send-keys",
            "-t",
            "%1",
            "-X",
            "-N",
            "2",
            "cursor-down",
            ";", //
            "send-keys",
            "-t",
            "%1",
            "-X",
            "start-of-line",
            ";", //
            "send-keys",
            "-t",
            "%1",
            "-X",
            "-N",
            "3",
            "cursor-right",
        ])
    );
}

#[test]
fn jump_in_copy_mode_top_left() {
    assert_eq!(
        jump_args("%1", PaneMode::Copy, TOP_LEFT, Focus::Keep),
        strings(&[
            "send-keys",
            "-t",
            "%1",
            "-X",
            "top-line",
            ";", //
            "send-keys",
            "-t",
            "%1",
            "-X",
            "start-of-line",
        ])
    );
}

#[test]
fn jump_selects_other_pane() {
    assert_eq!(
        jump_args("%2", PaneMode::Copy, TOP_LEFT, Focus::Select)[..5],
        strings(&["select-pane", "-t", "%2", ";", "send-keys"])
    );
}

#[test]
fn popup_covers_the_pane() {
    assert_eq!(
        popup_args("/dev/pts/3", "%1", Cover::Pane, (80, 24), "cmd"),
        strings(&[
            "display-popup",
            "-B",
            "-E",
            "-c",
            "/dev/pts/3",
            "-t",
            "%1",
            "-x",
            "#{popup_pane_left}",
            "-y",
            "#{popup_pane_bottom}",
            "-w",
            "80",
            "-h",
            "24",
            "cmd",
        ])
    );
}

#[test]
fn popup_covers_the_window() {
    // The window starts pane_left columns left of the pane; its bottom is
    // below the pane bottom by the rows under the pane.
    assert_eq!(
        popup_args("/dev/pts/3", "%1", Cover::Window, (80, 23), "cmd")[7..11],
        strings(&[
            "-x",
            "#{e|-:#{popup_pane_left},#{pane_left}}",
            "-y",
            "#{e|+:#{popup_pane_bottom},#{e|-:#{e|-:#{window_height},#{pane_top}},#{pane_height}}}",
        ])
    );
}

#[test]
fn message_args_per_kind() {
    assert_eq!(
        message_args("c", &Message::Prompt("a#b")),
        strings(&["display-message", "-c", "c", "-d", "0", "a##b"])
    );
    assert_eq!(
        message_args("c", &Message::Clear),
        strings(&["display-message", "-c", "c", "-d", "1", ""])
    );
    assert_eq!(
        message_args("c", &Message::Error("bad #1")),
        strings(&["display-message", "-c", "c", "bad ##1"])
    );
}

#[test]
fn quote_makes_one_shell_word() {
    assert_eq!(quote("/usr/bin/tmux-flash"), "'/usr/bin/tmux-flash'");
    assert_eq!(quote("it's"), r"'it'\''s'");
}
