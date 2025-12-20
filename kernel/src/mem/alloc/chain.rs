use core::alloc::{GlobalAlloc, Layout};
use core::mem::{align_of, size_of};
use core::ptr::NonNull;

use spin::Mutex;

use super::super::PGSIZE;
use crate::mem::PhysFrame;

const HEADER_SIZE: usize = size_of::<AllocHeader>();
const MIN_NODE_SIZE: usize = size_of::<ListNode>();

#[repr(C)]
struct ListNode {
    size: usize,
    next: Option<NonNull<ListNode>>, // free-list next
}

impl ListNode {
    #[inline(always)]
    fn start(&self) -> usize {
        self as *const _ as usize
    }

    #[inline(always)]
    fn end(&self) -> usize {
        self.start() + self.size
    }
}

#[repr(C)]
struct AllocHeader {
    size: usize, // total bytes reserved for this allocation (header + payload)
}

pub struct ChainAllocator {
    free: Mutex<Option<NonNull<ListNode>>>,
}

unsafe impl Sync for ChainAllocator {}

impl ChainAllocator {
    pub const fn new() -> Self {
        Self { free: Mutex::new(None) }
    }

    #[inline(always)]
    const fn align_up(value: usize, align: usize) -> usize {
        assert!(align.is_power_of_two());
        (value + align - 1) & !(align - 1)
    }

    fn insert_region(&self, head: &mut Option<NonNull<ListNode>>, addr: usize, size: usize) {
        let aligned_start = Self::align_up(addr, align_of::<ListNode>());
        let aligned_end = addr.saturating_add(size);
        if aligned_end <= aligned_start {
            return;
        }

        let adjusted_size = aligned_end - aligned_start;
        if adjusted_size < MIN_NODE_SIZE {
            return;
        }

        let node_ptr = aligned_start as *mut ListNode;
        unsafe {
            node_ptr.write(ListNode { size: adjusted_size, next: None });
        }

        // Use raw pointer to iterate to avoid borrowing `head` for too long
        let mut cursor_ptr = head as *mut Option<NonNull<ListNode>>;

        loop {
            let cursor = unsafe { &mut *cursor_ptr };
            if let Some(mut cur_ptr) = *cursor {
                if cur_ptr.as_ptr() as usize >= aligned_start {
                    break;
                }
                cursor_ptr = unsafe { &mut cur_ptr.as_mut().next };
            } else {
                break;
            }
        }

        let cursor = unsafe { &mut *cursor_ptr };
        let next = *cursor;
        *cursor = Some(unsafe { NonNull::new_unchecked(node_ptr) });
        unsafe {
            (*node_ptr).next = next;
        }

        self.merge(cursor);
        self.merge_previous(head, aligned_start);
    }

    fn merge(&self, node_opt: &mut Option<NonNull<ListNode>>) {
        if let Some(mut node_ptr) = *node_opt {
            unsafe {
                let node = node_ptr.as_mut();
                while let Some(next_ptr) = node.next {
                    let next = next_ptr.as_ref();
                    if node.end() == next.start() {
                        node.size += next.size;
                        node.next = next.next;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn merge_previous(&self, head: &mut Option<NonNull<ListNode>>, addr: usize) {
        let mut cursor = head;
        while let Some(mut cur_ptr) = *cursor {
            let next_addr = unsafe { cur_ptr.as_ref().next.map(|n| n.as_ptr() as usize) };
            if next_addr == Some(addr) {
                self.merge(cursor);
                break;
            }
            cursor = unsafe { &mut cur_ptr.as_mut().next };
        }
    }

    fn alloc_from_list(
        &self,
        head: &mut Option<NonNull<ListNode>>,
        size: usize,
        align: usize,
    ) -> Option<*mut u8> {
        let mut cursor_ptr = head as *mut Option<NonNull<ListNode>>;

        loop {
            let cursor = unsafe { &mut *cursor_ptr };
            if let Some(mut node_ptr) = *cursor {
                let node = unsafe { node_ptr.as_mut() };
                let node_start = node.start();
                let node_end = node.end();

                let alloc_start = Self::align_up(node_start + HEADER_SIZE, align);

                if let Some(alloc_end) = alloc_start.checked_add(size) {
                    if alloc_end <= node_end {
                        let block_start = alloc_start - HEADER_SIZE;
                        let block_end = alloc_end;

                        let before = block_start.saturating_sub(node_start);
                        let after = node_end.saturating_sub(block_end);

                        let next = node.next;
                        *cursor = next;

                        if before >= MIN_NODE_SIZE {
                            self.insert_region(head, node_start, before);
                        }
                        if after >= MIN_NODE_SIZE {
                            self.insert_region(head, block_end, after);
                        }

                        unsafe {
                            (block_start as *mut AllocHeader)
                                .write(AllocHeader { size: block_end - block_start });
                        }
                        return Some(alloc_start as *mut u8);
                    }
                }
                cursor_ptr = &mut node.next;
            } else {
                break;
            }
        }
        None
    }
}

unsafe impl GlobalAlloc for ChainAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let request_size = layout.size();
        // Limit: a single allocation cannot exceed one page for now.
        if request_size + HEADER_SIZE > PGSIZE {
            return core::ptr::null_mut();
        }

        let align = layout.align().max(align_of::<ListNode>());
        let mut head = self.free.lock();

        loop {
            if let Some(ptr) = self.alloc_from_list(&mut head, request_size, align) {
                return ptr;
            }

            let region_pa = PhysFrame::alloc().map(|f| f.leak()).unwrap_or(0);
            if region_pa == 0 {
                return core::ptr::null_mut();
            }
            self.insert_region(&mut head, region_pa, PGSIZE);
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let header_ptr = (ptr as usize - HEADER_SIZE) as *mut AllocHeader;
        let header = unsafe { header_ptr.read() };
        let mut head = self.free.lock();
        self.insert_region(&mut head, header_ptr as usize, header.size);
    }
}
