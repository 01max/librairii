// Copyright 2026 Librairii contributors
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Own one original window opacity until its first frame is submitted.

use std::cell::RefCell;

pub(crate) fn arm<T>(owner: &RefCell<Option<T>>, create: impl FnOnce() -> Option<T>) {
    if owner.borrow().is_none() {
        // Lazy creation can re-enter show before the outer call submits a
        // frame. Replacing its live guard would save zero as the old opacity.
        let reveal = create();
        *owner.borrow_mut() = reveal;
    }
}

#[cfg(test)]
mod tests {
    use super::arm;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct Reveal {
        opacity: Rc<Cell<f64>>,
        original: f64,
    }

    impl Reveal {
        fn new(opacity: &Rc<Cell<f64>>) -> Self {
            let original = opacity.replace(0.0);
            Self {
                opacity: opacity.clone(),
                original,
            }
        }
    }

    impl Drop for Reveal {
        fn drop(&mut self) {
            self.opacity.set(self.original);
        }
    }

    #[test]
    fn nested_show_preserves_first_opacity_until_submission() {
        for original in [1.0, 0.75, 0.25] {
            let opacity = Rc::new(Cell::new(original));
            let owner = RefCell::new(None);
            let created = Cell::new(0);
            for _ in 0..2 {
                arm(&owner, || {
                    created.set(created.get() + 1);
                    Some(Reveal::new(&opacity))
                });
                assert_eq!(opacity.get(), 0.0);
            }
            assert_eq!(created.get(), 1);
            drop(owner.take());
            assert_eq!(opacity.get(), original);
            arm(&owner, || Some(Reveal::new(&opacity)));
            assert_eq!(opacity.get(), 0.0);
            drop(owner.take());
            assert_eq!(opacity.get(), original);
        }
    }

    #[test]
    fn unavailable_window_does_not_prevent_later_reveal() {
        let owner = RefCell::new(None);
        arm(&owner, || None::<u8>);
        assert!(owner.borrow().is_none());
        arm(&owner, || Some(7));
        assert_eq!(owner.take(), Some(7));
    }
}
