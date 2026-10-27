//! Presentation acceptance is tied to the actual library activation, not its
//! persisted UUID (independent clones deliberately preserve that UUID).
use librairii_app::lifecycle::LibraryGeneration;
#[derive(Debug, Default)]
pub struct ResponseGuard {
    generation: Option<LibraryGeneration>,
    request: u64,
}
impl ResponseGuard {
    pub fn activate(&mut self, generation: LibraryGeneration) {
        self.generation = Some(generation);
        self.request = 0;
    }
    pub fn expect(&mut self, request: u64) {
        self.request = request;
    }
    pub fn accepts(&self, generation: &LibraryGeneration, request: u64) -> bool {
        self.generation.as_ref() == Some(generation) && self.request == request
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use librairii_app::lifecycle::LifecycleGate;
    #[test]
    fn obsolete_activation_and_superseded_query_cannot_publish_into_current_view()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = LifecycleGate::new();
        let second = LifecycleGate::new();
        let old = first.enter()?.generation().clone();
        let current = second.enter()?.generation().clone();
        let mut guard = ResponseGuard::default();
        guard.activate(current.clone());
        guard.expect(7);
        assert!(!guard.accepts(&old, 7));
        assert!(!guard.accepts(&current, 6));
        assert!(!guard.accepts(&current, 8));
        assert!(guard.accepts(&current, 7));
        Ok(())
    }
}
