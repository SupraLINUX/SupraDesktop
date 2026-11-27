//! Internal frame callback scheduler. Surface owns uncommitted requests;
//! this module owns committed FIFO queues and an index of live identities.
use std::collections::{BTreeMap, VecDeque};
use crate::{WindowId, commit::Error};

// Experimental resource bounds, not Wayland wire limits.
const MAX_LIVE_PER_SURFACE: usize = 512;
const MAX_LIVE_TOTAL: usize = 16384;

#[derive(Default)]
pub(crate) struct Frames {
    live: BTreeMap<u64, WindowId>,
    queues: BTreeMap<WindowId, VecDeque<(u64, u64)>>,
    // Earliest generation of the current mapping epoch. Old render work
    // must not signal callbacks after a null attach and subsequent remap.
    mapped_from: BTreeMap<WindowId, u64>,
}

impl Frames {
    pub(crate) fn validate_reservation(&self, window: WindowId, callback: u64, pending: usize) -> Result<(), Error> {
        if self.live.contains_key(&callback) { return Err(Error::DuplicateCallback); }
        let committed = self.queues.get(&window).map_or(0, VecDeque::len);
        if pending + committed >= MAX_LIVE_PER_SURFACE || self.live.len() >= MAX_LIVE_TOTAL {
            return Err(Error::CallbackCapacity);
        }
        Ok(())
    }

    pub(crate) fn reserve(&mut self, window: WindowId, callback: u64) {
        let previous = self.live.insert(callback, window);
        debug_assert!(previous.is_none());
    }

    pub(crate) fn mapped(&mut self, window: WindowId, generation: u64) {
        self.mapped_from.insert(window, generation);
    }

    pub(crate) fn unmapped(&mut self, window: WindowId) {
        self.mapped_from.remove(&window);
    }

    pub(crate) fn committed(&mut self, window: WindowId, generation: u64, callbacks: &[u64]) {
        if callbacks.is_empty() { return; }
        let queue = self.queues.entry(window).or_default();
        for &id in callbacks {
            debug_assert_eq!(self.live.get(&id), Some(&window));
            queue.push_back((generation, id));
        }
    }

    pub(crate) fn ready(&mut self, window: WindowId, generation: u64, visible: bool) -> Vec<u64> {
        if !visible || self.mapped_from.get(&window).is_none_or(|&first| generation < first) {
            return Vec::new();
        }
        let Some(queue) = self.queues.get_mut(&window) else { return Vec::new(); };
        let mut ready = Vec::new();
        while queue.front().is_some_and(|&(g, _)| g <= generation) {
            let (_, id) = queue.pop_front().expect("front checked");
            self.live.remove(&id);
            ready.push(id);
        }
        if queue.is_empty() { self.queues.remove(&window); }
        ready
    }

    pub(crate) fn cancel(&mut self, window: WindowId, pending: &[u64]) -> Vec<u64> {
        self.mapped_from.remove(&window);
        for id in pending { self.live.remove(id); }
        let committed: Vec<u64> = self.queues.remove(&window).into_iter().flatten().map(|(_, id)| id).collect();
        for id in &committed { self.live.remove(id); }
        committed
    }
}
