//! Generic list primitive wrapping ratatui's stateful [`List`].
//!
//! `List<T>` owns its items, its [`ListState`], and an optional filter
//! string. Selection state is expressed over the *filtered* subset so
//! `selected_item()` always returns something the user could see;
//! callers wanting the original-list index can search back through
//! `items()`.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List as RList, ListItem, ListState},
};

/// A generic scrollable list. The label callback is caller-provided at
/// render time so `List<T>` doesn't force a Display or ToString bound
/// on `T`.
pub struct List<T> {
    items: Vec<T>,
    state: ListState,
    filter: String,
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            state: ListState::default(),
            filter: String::new(),
        }
    }
}

impl<T> List<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }
        Self {
            items,
            state,
            filter: String::new(),
        }
    }

    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.state = ListState::default();
        if !self.items.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn set_filter(&mut self, filter: impl Into<String>) {
        let new_filter = filter.into();
        if new_filter == self.filter {
            // No-op reassignment — screens often re-push the current
            // filter on every render. Resetting selection here would
            // snap the cursor back to the top after every keystroke.
            return;
        }
        self.filter = new_filter;
        // Real change → the current index probably points into a
        // stale subset. Send it back to the top of the new subset.
        self.state.select(Some(0));
    }

    /// Index into the *filtered* subset. `None` when the subset is empty.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    /// Advance selection within the filtered subset. Wraps at the end.
    /// No-op on an empty filtered set.
    pub fn move_down<F>(&mut self, label: F)
    where
        F: Fn(&T) -> String,
    {
        let len = self.filtered_len(&label);
        if len == 0 {
            return;
        }
        let next = self.state.selected().map_or(0, |i| (i + 1) % len);
        self.state.select(Some(next));
    }

    /// Move selection back one within the filtered subset. Wraps at 0.
    pub fn move_up<F>(&mut self, label: F)
    where
        F: Fn(&T) -> String,
    {
        let len = self.filtered_len(&label);
        if len == 0 {
            return;
        }
        let prev = self
            .state
            .selected()
            .map_or(0, |i| if i == 0 { len - 1 } else { i - 1 });
        self.state.select(Some(prev));
    }

    /// The currently-selected item from the filtered subset, or `None`
    /// when nothing matches.
    pub fn selected_item<F>(&self, label: F) -> Option<&T>
    where
        F: Fn(&T) -> String,
    {
        let filtered: Vec<&T> = self.filtered(&label);
        let idx = self.state.selected()?;
        filtered.get(idx).copied()
    }

    /// Render the list into `area`. `label` maps each item to the string
    /// shown on its row.
    pub fn render<F>(&mut self, area: Rect, frame: &mut Frame, label: F)
    where
        F: Fn(&T) -> String,
    {
        let visible: Vec<ListItem> = self
            .filtered(&label)
            .iter()
            .map(|it| ListItem::new(label(it)))
            .collect();
        // Clamp selection to the current subset length so a stale
        // index from before a filter change never panics.
        let len = visible.len();
        if let Some(i) = self.state.selected()
            && (len == 0 || i >= len)
        {
            self.state.select(if len == 0 { None } else { Some(0) });
        }
        let list = RList::new(visible)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.state);
    }

    fn filtered<'a, F>(&'a self, label: &F) -> Vec<&'a T>
    where
        F: Fn(&T) -> String,
    {
        if self.filter.is_empty() {
            return self.items.iter().collect();
        }
        let needle = self.filter.to_ascii_lowercase();
        self.items
            .iter()
            .filter(|it| label(it).to_ascii_lowercase().contains(&needle))
            .collect()
    }

    fn filtered_len<F>(&self, label: &F) -> usize
    where
        F: Fn(&T) -> String,
    {
        if self.filter.is_empty() {
            self.items.len()
        } else {
            let needle = self.filter.to_ascii_lowercase();
            self.items
                .iter()
                .filter(|it| label(it).to_ascii_lowercase().contains(&needle))
                .count()
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    fn label(s: &&str) -> String {
        (*s).to_string()
    }

    #[test]
    fn empty_list_has_no_selection() {
        let list: List<&str> = List::new(Vec::new());
        assert!(list.selected().is_none());
    }

    #[test]
    fn nonempty_list_starts_at_zero() {
        let list = List::new(vec!["a", "b", "c"]);
        assert_eq!(list.selected(), Some(0));
    }

    #[test]
    fn move_down_wraps() {
        let mut list = List::new(vec!["a", "b", "c"]);
        list.move_down(label);
        assert_eq!(list.selected(), Some(1));
        list.move_down(label);
        assert_eq!(list.selected(), Some(2));
        list.move_down(label);
        assert_eq!(list.selected(), Some(0));
    }

    #[test]
    fn move_up_wraps() {
        let mut list = List::new(vec!["a", "b", "c"]);
        list.move_up(label);
        assert_eq!(list.selected(), Some(2));
        list.move_up(label);
        assert_eq!(list.selected(), Some(1));
    }

    #[test]
    fn selected_item_returns_current() {
        let list = List::new(vec!["alpha", "beta", "gamma"]);
        assert_eq!(list.selected_item(label), Some(&"alpha"));
    }

    #[test]
    fn filter_narrows_subset_and_resets_selection() {
        // "app" only matches "apple"; "an" matches "banana" only.
        // Use two distinct filters to prove subset selection resets.
        let mut list = List::new(vec!["apple", "banana", "cherry"]);
        list.set_filter("app");
        assert_eq!(list.selected_item(label), Some(&"apple"));
        assert!(list.selected().is_some());
        list.set_filter("an");
        assert_eq!(list.selected_item(label), Some(&"banana"));
    }

    #[test]
    fn empty_filter_result_returns_none() {
        let mut list = List::new(vec!["alpha", "beta"]);
        list.set_filter("zzz");
        assert!(list.selected_item(label).is_none());
    }

    #[test]
    fn filter_is_case_insensitive() {
        let mut list = List::new(vec!["Alpha", "BETA"]);
        list.set_filter("alp");
        assert_eq!(list.selected_item(label), Some(&"Alpha"));
        list.set_filter("BeTa");
        assert_eq!(list.selected_item(label), Some(&"BETA"));
    }

    #[test]
    fn set_filter_no_op_preserves_selection() {
        // Screens push the current filter on every render; a no-op
        // reassignment must not reset the cursor.
        let mut list = List::new(vec!["a", "b", "c"]);
        list.move_down(label); // selection = 1
        list.set_filter(""); // same as current → no reset
        assert_eq!(
            list.selected(),
            Some(1),
            "no-op filter must preserve selection"
        );
        list.set_filter("");
        list.set_filter("");
        assert_eq!(list.selected(), Some(1));
    }

    #[test]
    fn set_items_resets_state() {
        let mut list = List::new(vec!["a", "b", "c"]);
        list.move_down(label);
        list.move_down(label);
        list.set_items(vec!["x", "y"]);
        assert_eq!(list.selected(), Some(0));
    }

    #[test]
    fn render_into_test_backend_shows_items() {
        let backend = TestBackend::new(20, 5);
        let mut term = Terminal::new(backend).unwrap();
        let mut list = List::new(vec!["alpha", "beta"]);
        term.draw(|frame| list.render(frame.area(), frame, label))
            .unwrap();
        let buf = term.backend().buffer();
        let full: String = (0..5)
            .map(|y| {
                (0..20)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(full.contains("alpha"), "expected alpha in buffer: {full}");
        assert!(full.contains("beta"), "expected beta in buffer: {full}");
    }
}
