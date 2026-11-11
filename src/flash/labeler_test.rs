use super::*;
use crate::flash::matcher::find_matches;
use crate::flash::screen::Screen;

const WIDTH: usize = 40;

fn at(row: usize, col: usize) -> Point {
    Point { row, col }
}

fn matches(text: &str, pattern: &str) -> Vec<Match> {
    panes(&[text], pattern)
}

/// The matches of all panes, the first pane first, as the session has them.
fn panes(texts: &[&str], pattern: &str) -> Vec<Match> {
    texts
        .iter()
        .enumerate()
        .flat_map(|(pane, text)| {
            let height = text.lines().count().max(1);
            find_matches(&Screen::parse(text, WIDTH, height), pane, pattern)
        })
        .collect()
}

fn label(labels: &str, text: &str, pattern: &str, cursor: Point) -> Vec<Option<char>> {
    Labeler::new(labels).assign(&matches(text, pattern), cursor, WIDTH)
}

#[test]
fn pool_lower_then_upper() {
    assert_eq!(
        label("ab", "x x x x x\n", "x", at(0, 0)),
        [Some('a'), Some('b'), Some('A'), Some('B'), None]
    );
}

#[test]
fn pool_drops_duplicates() {
    assert_eq!(
        label("aab", "x x x\n", "x", at(0, 0)),
        [Some('a'), Some('b'), Some('A')]
    );
}

#[test]
fn skips_both_cases_of_next() {
    // Next after the matches: "b" and "C". Left: a, d, A, D.
    assert_eq!(
        label("abcd", "ab aC\n", "a", at(0, 0)),
        [Some('a'), Some('d')]
    );
}

#[test]
fn nearest_gets_first_label() {
    assert_eq!(
        label("cd", "al xxxxxxxxx al\n", "al", at(0, 14)),
        [Some('d'), Some('c')]
    );
}

#[test]
fn tie_keeps_reading_order() {
    assert_eq!(
        label("cd", "al xx al\n", "al", at(0, 3)),
        [Some('c'), Some('d')]
    );
}

#[test]
fn distance_counts_whole_rows() {
    // Row 0 col 10 is 10 cells away, row 1 col 0 is WIDTH cells away.
    assert_eq!(
        label("cd", "xxxxxxxxxxal\nal\n", "al", at(0, 0)),
        [Some('c'), Some('d')]
    );
}

#[test]
fn lowercase_label_stays() {
    let found = matches("al al\n", "al");
    let mut labeler = Labeler::new("cd");

    let first = labeler.assign(&found, at(0, 4), WIDTH);
    let second = labeler.assign(&found, at(0, 0), WIDTH);

    assert_eq!(first, [Some('d'), Some('c')]);
    assert_eq!(second, first);
}

#[test]
fn uppercase_label_does_not_stay() {
    let found = matches("x x x\n", "x");
    let mut labeler = Labeler::new("a");

    let first = labeler.assign(&found, at(0, 4), WIDTH);
    let second = labeler.assign(&found, at(0, 0), WIDTH);

    assert_eq!(first, [None, Some('A'), Some('a')]);
    assert_eq!(second, [Some('A'), None, Some('a')]);
}

#[test]
fn kept_label_goes_if_continued() {
    let mut labeler = Labeler::new("bc");

    let first = labeler.assign(&matches("ax ay\n", "a"), at(0, 4), WIDTH);

    // Now "b" follows the first match: b and B are skipped.
    let second = labeler.assign(&matches("ab ay\n", "a"), at(0, 4), WIDTH);

    assert_eq!(first, [Some('c'), Some('b')]);
    assert_eq!(second, [Some('c'), Some('C')]);
}

#[test]
fn no_matches_no_labels() {
    assert_eq!(label("ab", "x\n", "z", at(0, 0)), []);
}

#[test]
fn cursor_pane_comes_first() {
    // The other pane's match is in the cursor's row and column, but flash.nvim
    // labels the current window first.
    let found = panes(&["xxxxxxxxxxal\n", "al\n"], "al");

    assert_eq!(
        Labeler::new("cd").assign(&found, at(0, 0), WIDTH),
        [Some('c'), Some('d')]
    );
}

#[test]
fn other_panes_in_reading_order() {
    // Distance counts in the cursor pane only.
    let found = panes(&["\n", "al xxxxxxxxx al\n"], "al");

    assert_eq!(
        Labeler::new("cd").assign(&found, at(0, 14), WIDTH),
        [Some('c'), Some('d')]
    );
}

#[test]
fn skip_rule_spans_panes() {
    // "c" follows the match in the other pane: c and C are skipped.
    let found = panes(&["ax\n", "ac\n"], "a");

    assert_eq!(
        Labeler::new("cd").assign(&found, at(0, 0), WIDTH),
        [Some('d'), Some('D')]
    );
}

#[test]
fn kept_label_is_per_pane() {
    let mut labeler = Labeler::new("cd");

    let first = labeler.assign(&panes(&["\n", "al\n"], "a"), at(0, 0), WIDTH);

    // The same row and index in the cursor pane is another position.
    let second = labeler.assign(&panes(&["al\n", "al\n"], "a"), at(0, 0), WIDTH);

    assert_eq!(first, [Some('c')]);
    assert_eq!(second, [Some('d'), Some('c')]);
}
