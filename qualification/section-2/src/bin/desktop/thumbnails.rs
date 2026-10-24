//! Bounded synthetic PNG decode probe; no library, profile or external inputs.
use super::StoryRow;
use image::{ImageEncoder, ImageFormat, RgbImage};
use slint::{Model, ModelRc, ModelTracker, SharedPixelBuffer, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const CACHE: usize = 64;
const QUEUE: usize = 16;
type Reply = (usize, Result<Vec<u8>, String>, u64);

pub struct Loader {
    request: SyncSender<usize>,
    result: Receiver<Reply>,
    pending: HashSet<usize>,
    deferred: VecDeque<usize>,
    images: HashMap<usize, slint::Image>,
    lru: VecDeque<usize>,
    models: Vec<Weak<VecModel<StoryRow>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    pub decoded: HashSet<usize>,
    samples: VecDeque<u64>,
    pub peak_cache: usize,
    pub failure: Option<String>,
}
impl Loader {
    pub fn new() -> Self {
        let (request, requests) = mpsc::sync_channel(QUEUE);
        let (results, result) = mpsc::sync_channel(QUEUE);
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        let worker = thread::spawn(move || {
            while !cancel.load(Ordering::Acquire) {
                let id = match requests.recv_timeout(Duration::from_millis(10)) {
                    Ok(id) => id,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => return,
                };
                // Encode outside the decode measurement, on the owned worker.
                let pixels = RgbImage::from_fn(48, 64, |x, y| {
                    image::Rgb([
                        (45 + (id * 17 + y as usize) % 180) as u8,
                        (50 + id * 29 % 170) as u8,
                        (80 + x % 100) as u8,
                    ])
                });
                let mut png = Vec::new();
                let encoded = image::codecs::png::PngEncoder::new(&mut png).write_image(
                    pixels.as_raw(),
                    48,
                    64,
                    image::ExtendedColorType::Rgb8,
                );
                let started = Instant::now();
                let decoded = encoded.map_err(|e| e.to_string()).and_then(|_| {
                    image::load_from_memory_with_format(&png, ImageFormat::Png)
                        .map(|image| image.to_rgba8().into_raw())
                        .map_err(|e| e.to_string())
                });
                let mut reply = (id, decoded, started.elapsed().as_micros() as u64);
                loop {
                    if cancel.load(Ordering::Acquire) {
                        return;
                    }
                    match results.try_send(reply) {
                        Ok(()) => break,
                        Err(TrySendError::Full(value)) => {
                            reply = value;
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(TrySendError::Disconnected(_)) => return,
                    }
                }
            }
        });
        Self {
            request,
            result,
            pending: HashSet::new(),
            deferred: VecDeque::new(),
            images: HashMap::new(),
            lru: VecDeque::new(),
            models: Vec::new(),
            stop,
            worker: Some(worker),
            decoded: HashSet::new(),
            samples: VecDeque::new(),
            peak_cache: 0,
            failure: None,
        }
    }
    /// False means backpressure; retry on a later UI tick, never block paint.
    pub fn request(&mut self, id: usize) -> bool {
        if id >= 1_000 {
            return false;
        }
        if self.images.contains_key(&id) || self.pending.contains(&id) {
            return true;
        }
        if self.pending.len() > QUEUE * 2 {
            return false;
        }
        if self.request.try_send(id).is_err() {
            return false;
        }
        self.pending.insert(id);
        true
    }
    fn image(&mut self, id: usize) -> slint::Image {
        if let Some(image) = self.images.get(&id).cloned() {
            self.lru.retain(|&key| key != id);
            self.lru.push_back(id);
            return image;
        }
        if !self.request(id) && id < 1_000 && !self.deferred.contains(&id) {
            // The fixed fixture has only 1,000 identities. Preserve interest on
            // saturation so a blank visible row does not depend on scrolling.
            self.deferred.push_back(id);
        }
        slint::Image::default()
    }
    pub fn rows(loader: &Rc<RefCell<Self>>, rows: Vec<StoryRow>) -> ModelRc<StoryRow> {
        let inner = Rc::new(VecModel::from(rows));
        let mut state = loader.borrow_mut();
        state.models.retain(|model| model.strong_count() != 0);
        state.models.push(Rc::downgrade(&inner));
        drop(state);
        ModelRc::new(LazyRows {
            inner,
            loader: loader.clone(),
        })
    }
    pub fn poll(loader: &Rc<RefCell<Self>>) {
        let (models, changed) = {
            let mut state = loader.borrow_mut();
            let mut changed = HashSet::new();
            for _ in 0..QUEUE {
                let Ok((id, result, elapsed)) = state.result.try_recv() else {
                    break;
                };
                state.pending.remove(&id);
                let rgba = match result {
                    Ok(rgba) => rgba,
                    Err(error) => {
                        state.failure = Some(error);
                        continue;
                    }
                };
                state.decoded.insert(id);
                if state.samples.len() == 512 {
                    state.samples.pop_front();
                }
                state.samples.push_back(elapsed);
                if state.images.len() == CACHE
                    && let Some(old) = state.lru.pop_front()
                {
                    state.images.remove(&old);
                    // Mounted delegates may keep their bounded visible image.
                    // Do not invalidate it merely because the shared cache evicts:
                    // that would immediately request it again under pressure.
                }
                let pixels =
                    SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&rgba, 48, 64);
                state.images.insert(id, slint::Image::from_rgba8(pixels));
                state.lru.push_back(id);
                state.peak_cache = state.peak_cache.max(state.images.len());
                changed.insert(id);
            }
            for _ in 0..QUEUE {
                let Some(id) = state.deferred.front().copied() else {
                    break;
                };
                if !state.request(id) {
                    break;
                }
                state.deferred.pop_front();
            }
            if changed.is_empty() {
                return;
            }
            state.models.retain(|model| model.strong_count() != 0);
            (state.models.clone(), changed)
        };
        // Release the loader borrow before model notifications can read rows.
        for model in models.into_iter().filter_map(|model| model.upgrade()) {
            for row in 0..model.row_count() {
                if let Some(value) = model.row_data(row)
                    && changed.contains(&(value.id as usize))
                {
                    model.set_row_data(row, value);
                }
            }
        }
    }
    pub fn decode_p95_us(&self) -> u64 {
        let mut values: Vec<_> = self.samples.iter().copied().collect();
        values.sort_unstable();
        values
            .get((values.len() * 95).div_ceil(100).saturating_sub(1))
            .copied()
            .unwrap_or(0)
    }
    pub fn close(&mut self) -> Result<(), &'static str> {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "thumbnail worker panicked")?;
        }
        Ok(())
    }
}
impl Drop for Loader {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
struct LazyRows {
    inner: Rc<VecModel<StoryRow>>,
    loader: Rc<RefCell<Loader>>,
}
impl Model for LazyRows {
    type Data = StoryRow;
    fn row_count(&self) -> usize {
        self.inner.row_count()
    }
    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let mut value = self.inner.row_data(row)?;
        value.cover = self.loader.borrow_mut().image(value.id as usize);
        Some(value)
    }
    fn model_tracker(&self) -> &dyn ModelTracker {
        self.inner.model_tracker()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(loader: &Rc<RefCell<Loader>>, id: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "PNG worker stalled");
            loader.borrow_mut().request(id);
            Loader::poll(loader);
            assert!(loader.borrow().failure.is_none());
            if loader.borrow().images.contains_key(&id) {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn png_cache_evicts_least_recently_used_and_reloads_original_pixels() {
        let loader = Rc::new(RefCell::new(Loader::new()));
        for id in 0..CACHE {
            drain(&loader, id);
        }
        // A read refreshes recency: id 1, rather than id 0, must be evicted.
        let original = loader.borrow_mut().image(0).to_rgba8().unwrap();
        let original_one = loader.borrow().images[&1].to_rgba8().unwrap();
        drain(&loader, CACHE);
        assert!(loader.borrow().images.contains_key(&0));
        assert!(!loader.borrow().images.contains_key(&1));
        let retained = loader.borrow_mut().image(CACHE).to_rgba8().unwrap();
        assert_ne!(original.as_bytes(), retained.as_bytes());
        drain(&loader, 1);
        let reloaded_one = loader.borrow().images[&1].to_rgba8().unwrap();
        assert_eq!(original_one.as_bytes(), reloaded_one.as_bytes());
        assert_eq!(loader.borrow().images.len(), CACHE);
        assert_eq!(loader.borrow().lru.len(), CACHE);
        assert!(!loader.borrow().images.contains_key(&2));
        loader.borrow_mut().close().unwrap();
        assert!(loader.borrow().worker.is_none());
    }

    #[test]
    fn initially_visible_rows_retry_after_backpressure_without_incidental_reads() {
        let loader = Rc::new(RefCell::new(Loader::new()));
        let rows = Loader::rows(
            &loader,
            (0..48)
                .map(|id| StoryRow {
                    id,
                    title: "Synthetic".into(),
                    cover: slint::Image::default(),
                })
                .collect(),
        );
        for row in 0..48 {
            rows.row_data(row).unwrap();
        }
        assert!(!loader.borrow().deferred.is_empty());
        let deadline = Instant::now() + Duration::from_secs(5);
        while loader.borrow().decoded.len() < 48 {
            assert!(
                Instant::now() < deadline,
                "deferred visible rows never completed"
            );
            Loader::poll(&loader);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(loader.borrow().deferred.is_empty());
        assert_eq!(loader.borrow().images.len(), 48);
        loader.borrow_mut().close().unwrap();
    }

    #[test]
    fn full_queues_and_invalid_ids_remain_bounded_and_shutdown_joins() {
        let mut loader = Loader::new();
        assert!(!loader.request(1_000));
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut next = 0;
        while next < QUEUE * 2 + 1 {
            assert!(Instant::now() < deadline, "queues did not fill");
            if loader.request(next) {
                next += 1;
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
        assert_eq!(loader.pending.len(), QUEUE * 2 + 1);
        assert!(!loader.request(next));
        // Do not poll: the result queue fills and the worker must still stop.
        loader.close().unwrap();
        assert!(loader.worker.is_none());
    }
}
