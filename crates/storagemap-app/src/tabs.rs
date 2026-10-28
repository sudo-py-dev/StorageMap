//! Scan tabs: one is shown, the others are parked in order beside it.
//!
//! Kept as plain functions over the three pieces of state so the app can
//! keep its `scan` field (the shown tab) and the logic stays testable.

/// The open tabs: `shown` sits at position `active` among them.
pub struct Tabs<'a, T> {
    pub shown: &'a mut Option<T>,
    pub parked: &'a mut Vec<T>,
    pub active: &'a mut usize,
}

impl<T> Tabs<'_, T> {
    pub fn count(&self) -> usize {
        self.parked.len() + usize::from(self.shown.is_some())
    }

    /// Puts the shown tab back among the others, showing none.
    pub fn park(&mut self) {
        if let Some(tab) = self.shown.take() {
            let at = (*self.active).min(self.parked.len());
            self.parked.insert(at, tab);
            *self.active = at;
        }
    }

    pub fn switch(&mut self, index: usize) {
        if self.shown.is_some() && index == *self.active {
            return;
        }
        self.park();
        if index < self.parked.len() {
            *self.shown = Some(self.parked.remove(index));
            *self.active = index;
        }
    }

    /// Closes the tab at `index`; the one now in its place (or the last) is shown.
    pub fn close(&mut self, index: usize) {
        if index >= self.count() {
            return;
        }
        self.switch(index);
        *self.shown = None;
        if !self.parked.is_empty() {
            let next = index.min(self.parked.len() - 1);
            *self.shown = Some(self.parked.remove(next));
            *self.active = next;
        }
    }

    /// Shows `tab`, replacing the tab `same` matches or else added last.
    pub fn open(&mut self, tab: T, same: impl Fn(&T, &T) -> bool) {
        self.park();
        match self.parked.iter().position(|t| same(t, &tab)) {
            Some(i) => {
                self.parked.remove(i);
                *self.active = i;
            }
            None => *self.active = self.parked.len(),
        }
        *self.shown = Some(tab);
    }

    /// Removes and returns the first tab `pick` matches, shown or not.
    pub fn take(&mut self, pick: impl Fn(&T) -> bool) -> Option<T> {
        if self.shown.as_ref().is_some_and(&pick) {
            return self.shown.take();
        }
        let i = self.parked.iter().position(pick)?;
        if self.shown.is_some() && *self.active > i {
            *self.active -= 1;
        }
        Some(self.parked.remove(i))
    }
}

/// The tab at `index` among `shown` (at `active`) and `parked`, and
/// whether it is the shown one.
pub fn get<'a, T>(
    shown: Option<&'a T>,
    parked: &'a [T],
    active: usize,
    index: usize,
) -> Option<(&'a T, bool)> {
    match shown {
        Some(tab) if index == active => Some((tab, true)),
        Some(_) if index > active => parked.get(index - 1).map(|t| (t, false)),
        _ => parked.get(index).map(|t| (t, false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct State {
        shown: Option<&'static str>,
        parked: Vec<&'static str>,
        active: usize,
    }

    impl State {
        fn new() -> Self {
            State {
                shown: None,
                parked: Vec::new(),
                active: 0,
            }
        }
        fn tabs(&mut self) -> Tabs<'_, &'static str> {
            Tabs {
                shown: &mut self.shown,
                parked: &mut self.parked,
                active: &mut self.active,
            }
        }
        fn order(&mut self) -> Vec<(&'static str, bool)> {
            let count = self.tabs().count();
            (0..count)
                .filter_map(|i| get(self.shown.as_ref(), &self.parked, self.active, i))
                .map(|(t, a)| (*t, a))
                .collect()
        }
    }

    fn same(a: &&str, b: &&str) -> bool {
        a == b
    }

    #[test]
    fn opening_adds_tabs_in_order_and_replaces_the_same_folder() {
        let mut s = State::new();
        s.tabs().open("a", same);
        s.tabs().open("b", same);
        s.tabs().open("c", same);
        assert_eq!(s.order(), [("a", false), ("b", false), ("c", true)]);
        s.tabs().switch(0);
        s.tabs().open("b", same);
        assert_eq!(s.order(), [("a", false), ("b", true), ("c", false)]);
    }

    #[test]
    fn switching_keeps_positions() {
        let mut s = State::new();
        for t in ["a", "b", "c"] {
            s.tabs().open(t, same);
        }
        s.tabs().switch(1);
        assert_eq!(s.order(), [("a", false), ("b", true), ("c", false)]);
        s.tabs().park();
        assert_eq!(s.order(), [("a", false), ("b", false), ("c", false)]);
        s.tabs().switch(2);
        assert_eq!(s.order(), [("a", false), ("b", false), ("c", true)]);
    }

    #[test]
    fn closing_shows_the_neighbour() {
        let mut s = State::new();
        for t in ["a", "b", "c"] {
            s.tabs().open(t, same);
        }
        s.tabs().switch(1);
        s.tabs().close(1);
        assert_eq!(s.order(), [("a", false), ("c", true)]);
        s.tabs().close(0);
        assert_eq!(s.order(), [("c", true)]);
        s.tabs().close(0);
        assert!(s.order().is_empty());
        assert!(s.shown.is_none());
    }

    #[test]
    fn taking_a_parked_tab_keeps_the_shown_one_in_place() {
        let mut s = State::new();
        for t in ["a", "b", "c"] {
            s.tabs().open(t, same);
        }
        assert_eq!(s.tabs().take(|t| *t == "a"), Some("a"));
        assert_eq!(s.order(), [("b", false), ("c", true)]);
        assert_eq!(s.tabs().take(|t| *t == "c"), Some("c"));
        assert_eq!(s.order(), [("b", false)]);
    }
}
