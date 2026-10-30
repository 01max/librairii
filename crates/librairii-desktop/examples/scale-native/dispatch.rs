//! Qualification-only witnesses: search return and browser focus acknowledgement.
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Debug)]
pub struct Witnesses {
    origin: Instant,
    events: Vec<Value>,
    search: Option<Value>,
    browser: Option<Value>,
    searches: u64,
    focuses: u64,
    overflowed: bool,
}

impl Witnesses {
    pub fn install(ui: &librairii_ui::native::DesktopWindow) -> Rc<RefCell<Self>> {
        let observations = Rc::new(RefCell::new(Self {
            origin: Instant::now(),
            events: Vec::with_capacity(512),
            search: None,
            browser: None,
            searches: 0,
            focuses: 0,
            overflowed: false,
        }));
        let search = observations.clone();
        ui.on_qualification_search_dispatched(move |query| {
            let mut state = search.borrow_mut();
            state.searches += 1;
            let ack = json!({"kind":"search","sequence":state.searches,
                "query":query.as_str(),"native_ns":state.clock()});
            state.search = Some(ack.clone());
            state.record(ack);
        });
        let browser = observations.clone();
        ui.on_qualification_browser_focused(move || {
            let mut state = browser.borrow_mut();
            state.focuses += 1;
            let ack = json!({"kind":"browser-focus","sequence":state.focuses,
                "query":"","native_ns":state.clock()});
            state.browser = Some(ack.clone());
            state.record(ack);
        });
        observations
    }

    pub fn clock(&self) -> u64 {
        // The enclosing observer refuses after240s, far below the u64 range.
        self.origin.elapsed().as_nanos() as u64
    }

    fn record(&mut self, event: Value) {
        if self.events.len() < 4096 {
            self.events.push(event);
        } else {
            self.overflowed = true;
        }
        // Overflow cannot silently qualify: the complete joined log must
        // contain every measured witness and contiguous kind sequences.
    }

    pub fn summary(&self) -> Value {
        json!({"search":self.search,"browser_focus":self.browser,
            "search_sequence":self.searches,"browser_focus_sequence":self.focuses})
    }

    pub fn events(&self) -> std::io::Result<&[Value]> {
        if self.overflowed {
            return Err(std::io::Error::other(
                "native dispatch witness bound exceeded",
            ));
        }
        Ok(&self.events)
    }
}
