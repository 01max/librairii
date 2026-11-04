//! Temporary Task20 main-thread timing probe; no file writes during input.
use serde_json::{Value, json};
use std::time::Instant;

const STAGES: [&str; 13] = [
    "start",
    "controls",
    "resize",
    "menus",
    "snapshot",
    "projection",
    "imports",
    "page",
    "geometry",
    "capture",
    "state",
    "serialized",
    "end",
];

pub struct Tick {
    stamps: [u64; 13],
    previous_end: u64,
    search_sequence: u64,
}

pub struct Trace {
    origin: Instant,
    previous_end: u64,
    previous_search: u64,
    ticks: usize,
    overflowed: bool,
    retained: Vec<Tick>,
}

impl Trace {
    pub fn new(origin: Instant) -> Self {
        Self {
            origin,
            previous_end: 0,
            previous_search: 0,
            ticks: 0,
            overflowed: false,
            retained: Vec::with_capacity(4096),
        }
    }

    fn clock(&self) -> u64 {
        self.origin.elapsed().as_nanos() as u64
    }

    pub fn start(&self, search_sequence: u64) -> Tick {
        let mut tick = Tick {
            stamps: [0; 13],
            previous_end: self.previous_end,
            search_sequence,
        };
        self.mark(&mut tick, 0);
        tick
    }

    pub fn mark(&self, tick: &mut Tick, stage: usize) {
        tick.stamps[stage] = self.clock();
    }

    pub fn finish(&mut self, mut tick: Tick) {
        self.mark(&mut tick, 12);
        self.ticks += 1;
        let duration = tick.stamps[12] - tick.stamps[0];
        let gap = tick.stamps[0] - tick.previous_end;
        let selected = duration >= 20_000_000
            || gap >= 50_000_000
            || tick.search_sequence != self.previous_search;
        self.previous_end = tick.stamps[12];
        self.previous_search = tick.search_sequence;
        if selected {
            if self.retained.len() < 4096 {
                self.retained.push(tick);
            } else {
                self.overflowed = true;
            }
        }
    }

    pub fn report(&self) -> Value {
        json!({"format":"librairii.task20-temporary-main-thread-probe","schema":1,
            "clock":"same Instant origin as dispatch-events.json",
            "stages":STAGES,"zero_stamp":"stage not reached",
            "selection":"timer duration >=20ms, preceding gap >=50ms, or search sequence changed",
            "ticks":self.ticks,"overflowed":self.overflowed,
            "records":self.retained.iter().map(|tick|json!({"stamps_ns":tick.stamps,
                "previous_end_ns":tick.previous_end,"search_sequence":tick.search_sequence})).collect::<Vec<_>>()})
    }
}
