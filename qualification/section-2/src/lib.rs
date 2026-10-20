//! Synthetic qualification controller, deliberately independent of product policy.
pub const STORY_COUNT: usize = 1_000;
pub const REVIEW_PAGE: usize = 25;

#[derive(Clone, Debug)]
pub struct Story {
    pub id: usize,
    pub title: String,
}

pub fn stories() -> Vec<Story> {
    (0..STORY_COUNT)
        .map(|id| Story {
            id,
            title: format!("{id:04} · Étoiles — 日本語 — 故事"),
        })
        .collect()
}

/// Literal case-sensitive fixture filtering, NOT the product Unicode policy.
pub fn matching(query: &str) -> Vec<Story> {
    stories()
        .into_iter()
        .filter(|story| story.title.contains(query))
        .collect()
}

#[derive(Default, Debug)]
pub struct Review {
    // An explicit synthetic decision for each candidate; false means Not now.
    pub add: [bool; REVIEW_PAGE],
    pub applied: usize,
}

impl Review {
    pub fn apply(&mut self) -> usize {
        let count = self.add.iter().filter(|&&add| add).count();
        self.applied += count;
        self.add.fill(false);
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_fixture_retains_multilingual_titles_and_stable_selection_ids() {
        let all = stories();
        assert_eq!(all.len(), STORY_COUNT);
        let found = matching("0042");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, 42);
        assert!(found[0].title.contains("日本語"));
        assert!(matching("missing").is_empty());
    }

    #[test]
    fn review_requires_explicit_decisions_and_clears_them_after_apply() {
        let mut review = Review::default();
        assert_eq!(review.apply(), 0);
        review.add[4] = true;
        review.add[24] = true;
        assert_eq!(review.apply(), 2);
        assert_eq!(review.apply(), 0);
        assert_eq!(review.applied, 2);
    }
}
