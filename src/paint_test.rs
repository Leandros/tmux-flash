use super::*;
use crate::flash::screen::{Point, Screen};
use crate::flash::session::Key;

const LABELS: &str = "ctieobnrsgjduaxphlmwfvyz";
const TOP_LEFT: Point = Point { row: 0, col: 0 };

fn session(text: &str, width: usize, typed: &str) -> Session {
    let height = text.lines().count().max(1);
    let screen = Screen::parse(text, width, height);
    let mut session = Session::new(vec![screen], TOP_LEFT, LABELS);

    for key in typed.chars() {
        let _ = session.step(Key::Char(key));
    }
    session
}

/// The frame of a session with one pane that fills it.
fn frame(session: &Session) -> Vec<Vec<Span>> {
    let screen = &session.screens()[0];
    let (width, height) = (screen.width(), screen.lines().len());
    let layout = Layout {
        width,
        height,
        panes: vec![rect(0, 0, width, height)],
    };

    paint(session, &layout)
}

fn rect(left: usize, top: usize, width: usize, height: usize) -> Rect {
    Rect {
        left,
        top,
        width,
        height,
    }
}

fn span(text: &str, style: Style) -> Span {
    Span {
        text: text.to_owned(),
        style,
    }
}

#[test]
fn no_pattern_is_backdrop() {
    let rows = frame(&session("ab cd\nef\n", 10, ""));

    assert_eq!(
        rows,
        [
            vec![span("ab cd", Style::Backdrop)],
            vec![span("ef", Style::Backdrop)]
        ]
    );
}

#[test]
fn label_covers_next_cluster() {
    // Cursor at 0,0: the next match is the second one, the current match.
    let rows = frame(&session("ax ay\n", 10, "a"));

    assert_eq!(
        rows[0],
        [
            span("a", Style::Match),
            span("c", Style::Label),
            span(" ", Style::Backdrop),
            span("a", Style::Current),
            span("t", Style::Label),
        ]
    );
}

#[test]
fn label_after_text_end() {
    let rows = frame(&session("xx ab\n", 10, "ab"));

    assert_eq!(
        rows[0],
        [
            span("xx ", Style::Backdrop),
            span("ab", Style::Current),
            span("c", Style::Label),
        ]
    );
}

#[test]
fn label_at_right_edge() {
    let rows = frame(&session("xx ab\n", 5, "ab"));

    assert_eq!(
        rows[0],
        [
            span("xx ", Style::Backdrop),
            span("a", Style::Current),
            span("c", Style::Label),
        ]
    );
}

#[test]
fn label_on_wide_cluster() {
    let rows = frame(&session("a漢x\n", 10, "a"));

    assert_eq!(
        rows[0],
        [
            span("a", Style::Current),
            span("c ", Style::Label),
            span("x", Style::Backdrop),
        ]
    );
}

#[test]
fn unlabeled_match_highlighted() {
    let screen = Screen::parse("x x x\n", 10, 1);
    let mut s = Session::new(vec![screen], TOP_LEFT, "c");
    let _ = s.step(Key::Char('x'));

    let rows = frame(&s);

    assert_eq!(
        rows[0],
        [
            span("x", Style::Match),
            span("c", Style::Label),
            span("x", Style::Current),
            span("C", Style::Label),
            span("x", Style::Match),
        ]
    );
}

/// Two panes side by side: "ab" in 3 columns, a border, "cd" in 2.
fn side_by_side(left: &str, typed: &str) -> Vec<Vec<Span>> {
    let screens = vec![Screen::parse(left, 3, 1), Screen::parse("cd\n", 2, 1)];
    let mut s = Session::new(screens, TOP_LEFT, LABELS);
    for key in typed.chars() {
        let _ = s.step(Key::Char(key));
    }
    let layout = Layout {
        width: 6,
        height: 1,
        panes: vec![rect(0, 0, 3, 1), rect(4, 0, 2, 1)],
    };

    paint(&s, &layout)
}

#[test]
fn panes_at_their_columns() {
    assert_eq!(
        side_by_side("ab\n", ""),
        [vec![span("ab │cd", Style::Backdrop)]]
    );
}

#[test]
fn match_in_other_pane() {
    // Not the current match: that is in the cursor pane only. d follows the
    // match, so the label is c.
    assert_eq!(
        side_by_side("ab\n", "c"),
        [vec![
            span("ab │", Style::Backdrop),
            span("c", Style::Match),
            span("c", Style::Label),
        ]]
    );
}

#[test]
fn padding_counts_columns() {
    assert_eq!(
        side_by_side("漢\n", ""),
        [vec![span("漢 │cd", Style::Backdrop)]]
    );
}

#[test]
fn borders_join() {
    // A pane on the left, two on the right with a border between them.
    let screens = vec![
        Screen::parse("", 1, 3),
        Screen::parse("", 1, 1),
        Screen::parse("", 1, 1),
    ];
    let s = Session::new(screens, TOP_LEFT, LABELS);
    let layout = Layout {
        width: 3,
        height: 3,
        panes: vec![rect(0, 0, 1, 3), rect(2, 0, 1, 1), rect(2, 2, 1, 1)],
    };

    let rows: Vec<String> = paint(&s, &layout)
        .iter()
        .map(|row| row.iter().map(|span| span.text.as_str()).collect())
        .collect();

    assert_eq!(rows, [" │", " ├─", " │"]);
}
