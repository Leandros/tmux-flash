use super::*;

const LABELS: &str = "ctieobnrsgjduaxphlmwfvyz";
const WIDTH: usize = 40;
const TEXT: &str = "alpha alpine\n";

fn session(text: &str, cursor: Point, labels: &str) -> Session {
    panes(&[text], cursor, labels)
}

/// One screen per text; the first one holds the cursor.
fn panes(texts: &[&str], cursor: Point, labels: &str) -> Session {
    let screens = texts
        .iter()
        .map(|text| {
            let height = text.lines().count().max(1) + 1;
            Screen::parse(text, WIDTH, height)
        })
        .collect();

    Session::new(screens, cursor, labels)
}

fn at(row: usize, col: usize) -> Point {
    Point { row, col }
}

fn typed(session: &mut Session, text: &str) -> Step {
    let mut step = Step::Continue;
    for c in text.chars() {
        step = session.step(Key::Char(c));
    }
    step
}

fn jump(row: usize, index: usize) -> Step {
    jump_in(0, row, index)
}

fn jump_in(pane: usize, row: usize, index: usize) -> Step {
    Step::Jump(Target { pane, row, index })
}

#[test]
fn characters_extend_the_pattern() {
    let mut s = session(TEXT, at(1, 0), LABELS);

    assert_eq!(typed(&mut s, "al"), Step::Continue);
    assert_eq!(s.pattern(), "al");
    assert_eq!(s.matches().len(), 2);
}

#[test]
fn empty_pattern_has_no_labels() {
    let s = session(TEXT, at(1, 0), LABELS);

    assert_eq!(s.labels(), []);
}

#[test]
fn label_jumps_to_its_match() {
    let mut s = session(TEXT, at(1, 0), LABELS);
    let _ = typed(&mut s, "al");

    // After "a": alpine c, the last a of alpha t, alpha i. The labels stay
    // on their positions after "al".
    assert_eq!(s.labels(), [Some('i'), Some('c')]);
    assert_eq!(s.step(Key::Char('i')), jump(0, 0));
}

#[test]
fn uppercase_label_jumps() {
    let mut s = session("x x x\n", at(1, 0), "c");
    let _ = typed(&mut s, "x");

    assert_eq!(s.labels(), [None, Some('C'), Some('c')]);
    assert_eq!(s.step(Key::Char('C')), jump(0, 2));
}

#[test]
fn continuation_does_not_jump() {
    // p would be the first label, but p follows both matches.
    let mut s = session(TEXT, at(1, 0), "pc");
    let _ = typed(&mut s, "al");

    assert_eq!(s.step(Key::Char('p')), Step::Continue);
    assert_eq!(s.pattern(), "alp");
}

#[test]
fn enter_jumps_to_next_match() {
    let mut s = session(TEXT, at(0, 3), LABELS);
    let _ = typed(&mut s, "al");

    assert_eq!(s.current(), Some(1));
    assert_eq!(s.step(Key::Enter), jump(0, 6));
}

#[test]
fn enter_wraps_to_first() {
    let mut s = session(TEXT, at(1, 0), LABELS);
    let _ = typed(&mut s, "al");

    assert_eq!(s.step(Key::Enter), jump(0, 0));
}

#[test]
fn match_at_cursor_is_not_next() {
    let mut s = session(TEXT, at(0, 6), LABELS);
    let _ = typed(&mut s, "al");

    assert_eq!(s.current(), Some(0));
}

#[test]
fn enter_without_a_match_exits() {
    let mut s = session(TEXT, at(1, 0), LABELS);

    assert_eq!(s.step(Key::Enter), Step::Exit);
}

#[test]
fn backspace_removes_last() {
    let mut s = session(TEXT, at(1, 0), LABELS);
    let _ = typed(&mut s, "alp");

    assert_eq!(s.step(Key::Backspace), Step::Continue);
    assert_eq!(s.pattern(), "al");
    assert_eq!(s.matches().len(), 2);
}

#[test]
fn backspace_on_empty_pattern() {
    let mut s = session(TEXT, at(1, 0), LABELS);

    assert_eq!(s.step(Key::Backspace), Step::Continue);
    assert_eq!(s.pattern(), "");
}

#[test]
fn no_match_exits() {
    let mut s = session(TEXT, at(1, 0), LABELS);

    assert_eq!(typed(&mut s, "z"), Step::Exit);
}

#[test]
fn cancel_exits() {
    let mut s = session(TEXT, at(1, 0), LABELS);
    let _ = typed(&mut s, "al");

    assert_eq!(s.step(Key::Cancel), Step::Exit);
}

#[test]
fn label_jumps_into_other_pane() {
    let mut s = panes(&["beta\n", "x alpha\n"], at(0, 0), LABELS);
    let _ = typed(&mut s, "al");

    // After "a": beta c, then the other pane in reading order: alpha t,
    // alpha's last a i.
    assert_eq!(s.labels(), [Some('t')]);
    assert_eq!(s.step(Key::Char('t')), jump_in(1, 0, 2));
}

#[test]
fn match_in_other_pane_continues() {
    let mut s = panes(&["beta\n", "alpha\n"], at(0, 0), LABELS);

    assert_eq!(typed(&mut s, "al"), Step::Continue);
}

#[test]
fn enter_stays_in_cursor_pane() {
    // flash.nvim: the current match is in the current window only. The other
    // pane's match is on a later row than the cursor.
    let mut s = panes(&[TEXT, "x\nalpha\n"], at(0, 7), LABELS);
    let _ = typed(&mut s, "al");

    assert_eq!(s.current(), Some(0));
    assert_eq!(s.step(Key::Enter), jump(0, 0));
}

#[test]
fn enter_needs_cursor_pane_match() {
    let mut s = panes(&["beta\n", "alpha\n"], at(0, 0), LABELS);
    let _ = typed(&mut s, "al");

    assert_eq!(s.current(), None);
    assert_eq!(s.step(Key::Enter), Step::Exit);
}
