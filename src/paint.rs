//! The frame the popup shows: the pane text with backdrop, matches and
//! labels, as styled spans per row.

use unicode_width::UnicodeWidthStr;

use crate::flash::matcher::Match;
use crate::flash::screen::{Line, Screen};
use crate::flash::session::Session;

/// How a span looks; `theme` gives the colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Backdrop,
    Match,
    Current,
    Label,
}

/// Text of one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) text: String,
    pub(crate) style: Style,
}

/// One cell of the frame before spans of one style are joined.
type Painted = (String, Style);

/// A pane in the frame, in cells from the top left of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub(crate) left: usize,
    pub(crate) top: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl Rect {
    const fn contains(&self, x: usize, y: usize) -> bool {
        x >= self.left && x < self.left + self.width && y >= self.top && y < self.top + self.height
    }
}

/// The frame size and the place of each pane of the session in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// One per screen of the session, in the same order.
    pub(crate) panes: Vec<Rect>,
}

/// One row of spans per frame row.
pub(crate) fn paint(session: &Session, layout: &Layout) -> Vec<Vec<Span>> {
    let screens = session.screens();
    let mut panes: Vec<Vec<Vec<Painted>>> = screens
        .iter()
        .map(|screen| screen.lines().iter().map(backdrop).collect())
        .collect();

    for (index, found) in session.matches().iter().enumerate() {
        let style = if session.current() == Some(index) {
            Style::Current
        } else {
            Style::Match
        };
        let Some(row) = row_of(&mut panes, found) else {
            continue;
        };

        for cell in row.iter_mut().skip(found.index()).take(found.len()) {
            cell.1 = style;
        }
    }

    // Labels after the matches: a label covers a match that follows it.
    for (found, label) in session.matches().iter().zip(session.labels()) {
        let Some(label) = label else {
            continue;
        };
        let Some(screen) = screens.get(found.pane()) else {
            continue;
        };
        let Some(row) = row_of(&mut panes, found) else {
            continue;
        };

        put_label(row, screen, found, *label);
    }

    (0..layout.height)
        .map(|y| join(compose(&panes, layout, y)))
        .collect()
}

fn row_of<'a>(panes: &'a mut [Vec<Vec<Painted>>], found: &Match) -> Option<&'a mut Vec<Painted>> {
    panes.get_mut(found.pane())?.get_mut(found.row())
}

fn backdrop(line: &Line) -> Vec<Painted> {
    line.cells()
        .iter()
        .map(|cell| (cell.text().to_owned(), Style::Backdrop))
        .collect()
}

/// flash.nvim puts the label on the cluster after the match.
fn put_label(row: &mut Vec<Painted>, screen: &Screen, found: &Match, label: char) {
    let Some(line) = screen.lines().get(found.row()) else {
        return;
    };
    let cells = line.cells();
    let after = found.index() + found.len();

    if let Some(cell) = cells.get(after) {
        cover(row, after, label, cell.width());
        return;
    }

    let Some(last) = after.checked_sub(1) else {
        return;
    };
    let Some(last_cell) = cells.get(last) else {
        return;
    };

    // The match ends the text: the free column after it takes the label.
    if last_cell.col() + last_cell.width() < screen.width() {
        row.push((label.to_string(), Style::Label));
        return;
    }

    // At the right edge there is no free column.
    cover(row, last, label, last_cell.width());
}

/// Replace one cluster with the label; spaces fill the rest of its columns.
fn cover(row: &mut [Painted], index: usize, label: char, width: usize) {
    let Some(cell) = row.get_mut(index) else {
        return;
    };

    let fill = " ".repeat(width.saturating_sub(1));
    *cell = (format!("{label}{fill}"), Style::Label);
}

/// One frame row: each pane row at its column, the borders between them.
fn compose(panes: &[Vec<Vec<Painted>>], layout: &Layout, y: usize) -> Vec<Painted> {
    let mut row = Vec::new();
    let mut x = 0;

    while x < layout.width {
        let found = layout
            .panes
            .iter()
            .enumerate()
            .find(|(_, rect)| rect.contains(x, y));
        let Some((pane, rect)) = found else {
            row.push((border(layout, x, y).to_string(), Style::Backdrop));
            x += 1;
            continue;
        };

        let cells = panes
            .get(pane)
            .and_then(|rows| rows.get(y - rect.top))
            .map_or(&[][..], Vec::as_slice);
        row.extend_from_slice(cells);
        x = rect.left + rect.width;

        // The terminal erases after the last pane of a row; before a border
        // spaces must fill the pane width.
        let fill = rect.width.saturating_sub(columns(cells));
        if x < layout.width && fill > 0 {
            row.push((" ".repeat(fill), Style::Backdrop));
        }
    }

    row
}

/// Columns of the cells, by the rule of `Screen`: each takes one at least.
fn columns(cells: &[Painted]) -> usize {
    cells.iter().map(|(text, _)| text.width().max(1)).sum()
}

/// A line as tmux draws the pane borders: it joins the borders next to it.
fn border(layout: &Layout, x: usize, y: usize) -> char {
    let up = y.checked_sub(1).is_some_and(|y| is_border(layout, x, y));
    let down = is_border(layout, x, y + 1);
    let left = x.checked_sub(1).is_some_and(|x| is_border(layout, x, y));
    let right = is_border(layout, x + 1, y);

    match (up, down, left, right) {
        (true, true, true, true) => '┼',
        (true, true, true, false) => '┤',
        (true, true, false, true) => '├',
        (false, true, true, true) => '┬',
        (true, false, true, true) => '┴',
        (false, true, false, true) => '┌',
        (false, true, true, false) => '┐',
        (true, false, false, true) => '└',
        (true, false, true, false) => '┘',
        (_, _, false, false) => '│',
        _ => '─',
    }
}

fn is_border(layout: &Layout, x: usize, y: usize) -> bool {
    x < layout.width && y < layout.height && !layout.panes.iter().any(|rect| rect.contains(x, y))
}

fn join(row: Vec<Painted>) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();

    for (text, style) in row {
        if let Some(span) = spans.last_mut()
            && span.style == style
        {
            span.text.push_str(&text);
            continue;
        }

        spans.push(Span { text, style });
    }

    spans
}

#[cfg(test)]
#[path = "paint_test.rs"]
mod test;
