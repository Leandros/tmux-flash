//! One flash session: the pattern, its matches and labels, and what a key
//! does.

use super::labeler::Labeler;
use super::matcher::{Match, find_matches};
use super::screen::{CURSOR_PANE, Point, Screen};

/// A key, as the session sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    Char(char),
    Enter,
    Backspace,
    Cancel,
}

/// Where the cursor goes: the pane, the row, and the grapheme clusters
/// before the target on that row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) pane: usize,
    pub(crate) row: usize,
    pub(crate) index: usize,
}

/// What follows a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Continue,
    Jump(Target),
    Exit,
}

#[derive(Debug)]
pub(crate) struct Session {
    screens: Vec<Screen>,
    cursor: Point,
    labeler: Labeler,
    pattern: String,
    matches: Vec<Match>,
    labels: Vec<Option<char>>,
    current: Option<usize>,
}

impl Session {
    /// One screen per pane; the first one holds the cursor.
    pub(crate) fn new(screens: Vec<Screen>, cursor: Point, labels: &str) -> Self {
        Self {
            screens,
            cursor,
            labeler: Labeler::new(labels),
            pattern: String::new(),
            matches: Vec::new(),
            labels: Vec::new(),
            current: None,
        }
    }

    pub(crate) fn step(&mut self, key: Key) -> Step {
        match key {
            Key::Cancel => Step::Exit,
            Key::Enter => self.jump_to(self.current),
            Key::Backspace => {
                let _ = self.pattern.pop();
                self.update()
            }
            Key::Char(c) => {
                // The skip rule makes sure that no label continues the pattern.
                let labeled = self.labels.iter().position(|&label| label == Some(c));
                if labeled.is_some() {
                    return self.jump_to(labeled);
                }

                self.pattern.push(c);
                self.update()
            }
        }
    }

    fn jump_to(&self, index: Option<usize>) -> Step {
        let Some(found) = index.and_then(|index| self.matches.get(index)) else {
            return Step::Exit;
        };

        Step::Jump(Target {
            pane: found.pane(),
            row: found.row(),
            index: found.index(),
        })
    }

    fn update(&mut self) -> Step {
        self.matches = self
            .screens
            .iter()
            .enumerate()
            .flat_map(|(pane, screen)| find_matches(screen, pane, &self.pattern))
            .collect();
        let width = self.screens.get(CURSOR_PANE).map_or(0, Screen::width);
        self.labels = self.labeler.assign(&self.matches, self.cursor, width);
        self.current = next_after(&self.matches, self.cursor);

        // flash.nvim ends when a typed character leaves no match.
        if self.matches.is_empty() && !self.pattern.is_empty() {
            return Step::Exit;
        }

        Step::Continue
    }

    pub(crate) fn pattern(&self) -> &str {
        &self.pattern
    }

    /// One per pane, in the order of `new`.
    pub(crate) fn screens(&self) -> &[Screen] {
        &self.screens
    }

    pub(crate) fn matches(&self) -> &[Match] {
        &self.matches
    }

    /// One label or none per match.
    pub(crate) fn labels(&self) -> &[Option<char>] {
        &self.labels
    }

    /// The index of the match that Enter jumps to.
    pub(crate) const fn current(&self) -> Option<usize> {
        self.current
    }
}

#[cfg(test)]
#[path = "session_test.rs"]
mod test;

/// The first match after the cursor, else the first one (flash.nvim wraps).
/// Only matches in the cursor pane count, as flash.nvim finds the current
/// match in the current window only.
fn next_after(matches: &[Match], cursor: Point) -> Option<usize> {
    let mut own = matches
        .iter()
        .enumerate()
        .filter(|(_, found)| found.pane() == CURSOR_PANE);
    let (first, _) = own.clone().next()?;

    let after = own.find(|(_, found)| (found.row(), found.col()) > (cursor.row, cursor.col));

    Some(after.map_or(first, |(index, _)| index))
}
