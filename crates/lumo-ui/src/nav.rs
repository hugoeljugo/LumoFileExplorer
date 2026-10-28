//! Per-tab back/forward history.

use std::mem;
use std::path::{Path, PathBuf};

/// Oldest entries beyond this are dropped from the back stack.
const MAX_BACK: usize = 100;

#[derive(Debug, Clone)]
pub struct History {
    back: Vec<PathBuf>,
    current: PathBuf,
    forward: Vec<PathBuf>,
}

impl History {
    pub fn new(start: PathBuf) -> Self {
        Self {
            back: Vec::new(),
            current: start,
            forward: Vec::new(),
        }
    }

    pub fn current(&self) -> &Path {
        &self.current
    }

    /// Moves to `path`, clearing the forward stack. Visiting the current
    /// location again is a no-op.
    pub fn visit(&mut self, path: PathBuf) {
        if path == self.current {
            return;
        }
        self.back.push(mem::replace(&mut self.current, path));
        if self.back.len() > MAX_BACK {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Returns `false` if there is nothing to go back to.
    pub fn back(&mut self) -> bool {
        let Some(previous) = self.back.pop() else {
            return false;
        };
        self.forward.push(mem::replace(&mut self.current, previous));
        true
    }

    /// Returns `false` if there is nothing to go forward to.
    pub fn forward(&mut self) -> bool {
        let Some(next) = self.forward.pop() else {
            return false;
        };
        self.back.push(mem::replace(&mut self.current, next));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn back_and_forward_walk_the_stacks() {
        let mut h = History::new(p("a"));
        h.visit(p("b"));
        h.visit(p("c"));

        assert!(h.back());
        assert_eq!(h.current(), p("b"));
        assert!(h.back());
        assert_eq!(h.current(), p("a"));
        assert!(!h.back());
        assert!(h.forward());
        assert_eq!(h.current(), p("b"));
    }

    #[test]
    fn visiting_clears_forward() {
        let mut h = History::new(p("a"));
        h.visit(p("b"));
        h.back();
        h.visit(p("c"));

        assert!(!h.can_go_forward());
        assert!(h.back());
        assert_eq!(h.current(), p("a"));
    }

    #[test]
    fn revisiting_current_is_a_no_op() {
        let mut h = History::new(p("a"));
        h.visit(p("a"));
        assert!(!h.can_go_back());
    }

    #[test]
    fn back_stack_is_capped() {
        let mut h = History::new(p("0"));
        for i in 1..=MAX_BACK + 10 {
            h.visit(p(&i.to_string()));
        }
        let mut steps = 0;
        while h.back() {
            steps += 1;
        }
        assert_eq!(steps, MAX_BACK);
    }
}
