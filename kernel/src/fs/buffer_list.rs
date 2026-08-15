//! Type-safe doubly linked list for buffer cache
//! 
//! This module provides a type-safe implementation of a doubly linked list
//! using indices instead of raw pointers, improving safety and maintainability.

/// Type-safe buffer index
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BufferIndex(usize);

impl BufferIndex {
    /// Create a new BufferIndex, panics if out of bounds
    pub fn new(idx: usize, max_buffers: usize) -> Self {
        assert!(idx < max_buffers, "BufferIndex out of bounds: {} >= {}", idx, max_buffers);
        Self(idx)
    }

    /// Get the raw index value
    pub fn as_usize(self) -> usize {
        self.0
    }

    /// Create from usize without bounds check (unsafe, use with caution)
    pub const unsafe fn from_usize_unchecked(idx: usize) -> Self {
        Self(idx)
    }
}

/// Type-safe doubly linked list using indices
/// 
/// Note: This uses a fixed-size array. The max_buffers parameter is used
/// for bounds checking but the array size is fixed at compile time.
pub struct DoublyLinkedList<const MAX_BUFFERS: usize> {
    next: [Option<BufferIndex>; MAX_BUFFERS + 2],
    prev: [Option<BufferIndex>; MAX_BUFFERS + 2],
}

impl<const MAX_BUFFERS: usize> DoublyLinkedList<MAX_BUFFERS> {
    /// Create a new list
    pub const fn new() -> Self {
        Self {
            next: [None; MAX_BUFFERS + 2],
            prev: [None; MAX_BUFFERS + 2],
        }
    }

    /// Get head indices
    pub const fn head_inactive(&self) -> BufferIndex {
        unsafe { BufferIndex::from_usize_unchecked(MAX_BUFFERS) }
    }

    pub const fn head_active(&self) -> BufferIndex {
        unsafe { BufferIndex::from_usize_unchecked(MAX_BUFFERS + 1) }
    }

    /// Initialize the list with all buffers in the inactive list
    pub fn init(&mut self) {
        let head_active = self.head_active();
        let head_inactive = self.head_inactive();
        
        // Initialize heads to point to themselves
        self.next[head_active.as_usize()] = Some(head_active);
        self.prev[head_active.as_usize()] = Some(head_active);
        self.next[head_inactive.as_usize()] = Some(BufferIndex::new(0, MAX_BUFFERS));
        self.prev[head_inactive.as_usize()] = Some(BufferIndex::new(MAX_BUFFERS - 1, MAX_BUFFERS));

        // Link all buffers in inactive list
        for i in 0..MAX_BUFFERS {
            let idx = BufferIndex::new(i, MAX_BUFFERS);
            self.next[idx.as_usize()] = if i == MAX_BUFFERS - 1 {
                Some(head_inactive)
            } else {
                Some(BufferIndex::new(i + 1, MAX_BUFFERS))
            };
            self.prev[idx.as_usize()] = if i == 0 {
                Some(head_inactive)
            } else {
                Some(BufferIndex::new(i - 1, MAX_BUFFERS))
            };
        }
    }

    /// Insert a node at the head of a list
    pub fn insert_head(&mut self, head: BufferIndex, node: BufferIndex) {
        let first = self.next[head.as_usize()].unwrap_or(head);
        self.next[head.as_usize()] = Some(node);
        self.prev[node.as_usize()] = Some(head);
        self.next[node.as_usize()] = Some(first);
        self.prev[first.as_usize()] = Some(node);
    }

    /// Remove a node from its list
    pub fn remove(&mut self, node: BufferIndex) {
        let prev = self.prev[node.as_usize()].expect("Node not in list");
        let next = self.next[node.as_usize()].expect("Node not in list");
        self.next[prev.as_usize()] = Some(next);
        self.prev[next.as_usize()] = Some(prev);
    }

    /// Get the next node in the list
    pub fn next(&self, node: BufferIndex) -> Option<BufferIndex> {
        self.next[node.as_usize()]
    }

    /// Get the previous node in the list
    pub fn prev(&self, node: BufferIndex) -> Option<BufferIndex> {
        self.prev[node.as_usize()]
    }

    /// Get the last (LRU) node in the inactive list
    pub fn lru(&self) -> Option<BufferIndex> {
        let head_inactive = self.head_inactive();
        let prev = self.prev[head_inactive.as_usize()]?;
        if prev == head_inactive {
            None
        } else {
            Some(prev)
        }
    }

    /// Iterate over nodes in a list starting from head
    pub fn iter_from(&self, head: BufferIndex) -> ListIterator<'_, MAX_BUFFERS> {
        ListIterator {
            list: self,
            current: self.next[head.as_usize()],
            head,
        }
    }
}

/// Iterator over list nodes
pub struct ListIterator<'a, const MAX_BUFFERS: usize> {
    list: &'a DoublyLinkedList<MAX_BUFFERS>,
    current: Option<BufferIndex>,
    head: BufferIndex,
}

impl<'a, const MAX_BUFFERS: usize> Iterator for ListIterator<'a, MAX_BUFFERS> {
    type Item = BufferIndex;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        if current == self.head {
            return None;
        }
        self.current = self.list.next(current);
        Some(current)
    }
}
