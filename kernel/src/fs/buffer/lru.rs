//! LRU cache implementation using type-safe indices
//! 
//! This module provides a modern, type-safe implementation of the LRU cache
//! for buffer management, replacing manual linked list manipulation with
//! clearer abstractions.

use super::{Buffer, N_BUFFER};

/// Type-safe buffer index
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BufferId(usize);

impl BufferId {
    pub const fn new(idx: usize) -> Option<Self> {
        if idx < N_BUFFER {
            Some(Self(idx))
        } else {
            None
        }
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_index(self) -> usize {
        self.0
    }
}

/// Doubly-linked list node for LRU cache
#[derive(Debug, Clone, Copy)]
struct ListNode {
    prev: Option<BufferId>,
    next: Option<BufferId>,
}

impl ListNode {
    const fn new() -> Self {
        Self { prev: None, next: None }
    }
}

/// LRU cache structure with type-safe linked list
pub struct LRUCache {
    buffers: [Buffer; N_BUFFER],
    nodes: [ListNode; N_BUFFER],
    active_head: Option<BufferId>,
    inactive_head: Option<BufferId>,
}

impl LRUCache {
    pub const fn new() -> Self {
        Self {
            buffers: [const { Buffer::new() }; N_BUFFER],
            nodes: [const { ListNode::new() }; N_BUFFER],
            active_head: None,
            inactive_head: None,
        }
    }

    /// Get a reference to a buffer
    pub fn get_buffer(&self, id: BufferId) -> &Buffer {
        &self.buffers[id.as_index()]
    }

    /// Get a mutable reference to a buffer
    pub fn get_buffer_mut(&mut self, id: BufferId) -> &mut Buffer {
        &mut self.buffers[id.as_index()]
    }

    /// Remove a node from its current list
    fn remove_node(&mut self, id: BufferId) {
        let idx = id.as_index();
        let prev = self.nodes[idx].prev;
        let next = self.nodes[idx].next;

        // Update previous node or list head
        if let Some(prev_id) = prev {
            self.nodes[prev_id.as_index()].next = next;
        } else {
            if self.active_head == Some(id) {
                self.active_head = next;
            } else if self.inactive_head == Some(id) {
                self.inactive_head = next;
            }
        }

        // Update next node's prev pointer
        if let Some(next_id) = next {
            self.nodes[next_id.as_index()].prev = prev;
        }

        // Clear the removed node's links
        self.nodes[idx].prev = None;
        self.nodes[idx].next = None;
    }

    /// Insert a node at the head of the active list
    fn insert_active_head(&mut self, id: BufferId) {
        // Ensure it's detached from any list first
        self.remove_node(id);

        let old_head = self.active_head;
        let idx = id.as_index();

        // Link new head
        self.nodes[idx].prev = None;
        self.nodes[idx].next = old_head;

        // Fix previous head's prev pointer
        if let Some(h) = old_head {
            self.nodes[h.as_index()].prev = Some(id);
        }
        self.active_head = Some(id);
    }

    /// Insert a node at the head of the inactive list (MRU position)
    fn insert_inactive_head(&mut self, id: BufferId) {
        // Ensure it's detached from any list first
        self.remove_node(id);

        let old_head = self.inactive_head;
        let idx = id.as_index();

        // Link new head
        self.nodes[idx].prev = None;
        self.nodes[idx].next = old_head;

        // Fix previous head's prev pointer
        if let Some(h) = old_head {
            self.nodes[h.as_index()].prev = Some(id);
        }
        self.inactive_head = Some(id);
    }

    /// Get the LRU buffer (tail of inactive list)
    fn get_lru(&self) -> Option<BufferId> {
        let mut current = self.inactive_head?;
        loop {
            let node = &self.nodes[current.as_index()];
            if node.next.is_none() {
                return Some(current);
            }
            current = node.next?;
        }
    }

    /// Find a buffer in the active list
    pub fn find_active(&self, dev: u32, blockno: u32) -> Option<BufferId> {
        let mut current = self.active_head?;
        loop {
            let buf = &self.buffers[current.as_index()];
            if buf.dev == dev && buf.block_no == blockno {
                return Some(current);
            }
            let node = &self.nodes[current.as_index()];
            current = node.next?;
        }
    }

    /// Find a buffer in the inactive list
    pub fn find_inactive(&self, dev: u32, blockno: u32) -> Option<BufferId> {
        let mut current = self.inactive_head?;
        loop {
            let buf = &self.buffers[current.as_index()];
            if buf.dev == dev && buf.block_no == blockno {
                return Some(current);
            }
            let node = &self.nodes[current.as_index()];
            current = node.next?;
        }
    }

    /// Move a buffer from inactive to active list
    pub fn promote_to_active(&mut self, id: BufferId) {
        {
            let buf = self.get_buffer_mut(id);
            buf.refcnt += 1;
        }
        self.insert_active_head(id);
    }

    /// Move a buffer from active to inactive list (when refcnt becomes 0)
    pub fn demote_to_inactive(&mut self, id: BufferId) {
        self.insert_inactive_head(id);
    }

    /// Recycle the LRU buffer for a new block
    pub fn recycle_lru(&mut self, dev: u32, blockno: u32) -> BufferId {
        let lru = self.get_lru().expect("No buffers available");
        {
            let buf = self.get_buffer_mut(lru);
            debug_assert_eq!(buf.refcnt, 0, "LRU buffer should have refcnt=0");
            buf.dev = dev;
            buf.block_no = blockno;
            buf.valid = false;
            buf.refcnt = 1;
            buf.locked = true;
        }
        self.insert_active_head(lru);
        lru
    }

    /// Initialize the cache with all buffers in the inactive list
    pub fn init(&mut self) {
        // Clear all nodes
        for node in &mut self.nodes {
            *node = ListNode::new();
        }

        // Build initial inactive list (all buffers)
        if N_BUFFER == 0 {
            return;
        }

        // Link all buffers in a chain
        for i in 0..N_BUFFER {
            let id = BufferId::new(i).unwrap();
            let node = &mut self.nodes[i];
            node.prev = if i > 0 { BufferId::new(i - 1) } else { None };
            node.next = if i < N_BUFFER - 1 { BufferId::new(i + 1) } else { None };
        }

        self.inactive_head = BufferId::new(0);
        self.active_head = None;
    }

    /// Iterate over active list (for debugging)
    pub fn iter_active(&self) -> ActiveIter {
        ActiveIter {
            cache: self,
            current: self.active_head,
        }
    }

    /// Iterate over inactive list (for debugging)
    pub fn iter_inactive(&self) -> InactiveIter {
        InactiveIter {
            cache: self,
            current: self.inactive_head,
        }
    }
}

/// Iterator over active buffers
pub struct ActiveIter<'a> {
    cache: &'a LRUCache,
    current: Option<BufferId>,
}

impl<'a> Iterator for ActiveIter<'a> {
    type Item = BufferId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let node = &self.cache.nodes[id.as_index()];
        self.current = node.next;
        Some(id)
    }
}

/// Iterator over inactive buffers
pub struct InactiveIter<'a> {
    cache: &'a LRUCache,
    current: Option<BufferId>,
}

impl<'a> Iterator for InactiveIter<'a> {
    type Item = BufferId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let node = &self.cache.nodes[id.as_index()];
        self.current = node.next;
        Some(id)
    }
}
