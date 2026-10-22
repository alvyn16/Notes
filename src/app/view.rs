/// Notes open rendered. Find may temporarily expose the editor without changing
/// the view the reader chose before searching.
#[derive(Default)]
pub(super) struct View {
    pub editing: bool,
    before_find: Option<bool>,
}

impl View {
    pub fn read(&mut self) {
        self.editing = false;
        self.before_find = None;
    }

    pub fn toggle(&mut self) {
        self.editing = !self.editing;
        self.before_find = None;
    }

    pub fn find(&mut self) {
        self.before_find.get_or_insert(self.editing);
        self.editing = true;
    }

    pub fn close_find(&mut self) {
        if let Some(editing) = self.before_find.take() {
            self.editing = editing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::View;

    #[test]
    fn repeated_find_and_replace_return_to_reading() {
        let mut view = View::default();
        assert!(!view.editing);
        view.find();
        view.find();
        assert!(view.editing);
        view.close_find();
        assert!(!view.editing);
    }

    #[test]
    fn find_preserves_explicit_editing_and_navigation_clears_it() {
        let mut view = View::default();
        view.toggle();
        view.find();
        view.close_find();
        assert!(view.editing);
        view.find();
        view.read();
        view.close_find();
        assert!(!view.editing);
    }
}
