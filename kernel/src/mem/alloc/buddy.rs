use crate::mem::{PhysAddr, PhysFrame};
use core::alloc::{GlobalAlloc, Layout};
use core::cmp::{max, min};
use core::mem::size_of;
use core::ptr::{self, NonNull};
use spin::Mutex;

// Min block size = 8 bytes (enough for next pointer)
const MIN_ORDER: usize = 3;
// Max block size = 4096 bytes (Page size)
const MAX_ORDER: usize = 12;
const ORDER_COUNT: usize = MAX_ORDER - MIN_ORDER + 1;

#[repr(C)]
struct FreeBlock {
    next: Option<NonNull<FreeBlock>>,
}

pub struct BuddyAllocator {
    free_lists: [Mutex<Option<NonNull<FreeBlock>>>; ORDER_COUNT],
}

unsafe impl Sync for BuddyAllocator {}

impl BuddyAllocator {
    pub const fn new() -> Self {
        Self { free_lists: [const { Mutex::new(None) }; ORDER_COUNT] }
    }

    fn order_for_size(size: usize) -> usize {
        let size = max(size, 1 << MIN_ORDER);
        let mut order = MIN_ORDER;
        while (1 << order) < size {
            order += 1;
        }
        order
    }

    fn list_index(order: usize) -> usize {
        order - MIN_ORDER
    }

    unsafe fn push_block(&self, ptr: *mut u8, order: usize) {
        let idx = Self::list_index(order);
        let mut list = self.free_lists[idx].lock();
        let block = ptr as *mut FreeBlock;
        unsafe {
            (*block).next = *list;
            *list = Some(NonNull::new_unchecked(block));
        }
    }

    unsafe fn pop_block(&self, order: usize) -> Option<*mut u8> {
        let idx = Self::list_index(order);
        let mut list = self.free_lists[idx].lock();
        if let Some(ptr) = *list {
            let block = ptr.as_ptr();
            unsafe {
                *list = (*block).next;
            }
            Some(block as *mut u8)
        } else {
            None
        }
    }

    unsafe fn remove_block(&self, ptr: *mut u8, order: usize) -> bool {
        let idx = Self::list_index(order);
        let mut list = self.free_lists[idx].lock();
        let mut cursor = &mut *list;

        while let Some(mut node_ptr) = *cursor {
            if node_ptr.as_ptr() as *mut u8 == ptr {
                unsafe {
                    *cursor = node_ptr.as_mut().next;
                }
                return true;
            }
            unsafe {
                cursor = &mut node_ptr.as_mut().next;
            }
        }
        false
    }
}

unsafe impl GlobalAlloc for BuddyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = max(layout.size(), size_of::<FreeBlock>());
        let size = max(size, layout.align());
        let order = Self::order_for_size(size);

        if order > MAX_ORDER {
            return ptr::null_mut();
        }

        for i in order..=MAX_ORDER {
            if let Some(ptr) = unsafe { self.pop_block(i) } {
                let mut current_order = i;
                let current_ptr = ptr as usize;

                while current_order > order {
                    current_order -= 1;
                    let buddy_ptr = current_ptr + (1 << current_order);
                    unsafe { self.push_block(buddy_ptr as *mut u8, current_order) };
                }
                return ptr;
            }
        }

        let page =
            PhysFrame::alloc().map(|f| f.leak().as_mut_ptr::<u8>()).unwrap_or(ptr::null_mut());
        if page.is_null() {
            return ptr::null_mut();
        }

        let mut current_order = MAX_ORDER;
        let current_ptr = page as usize;

        while current_order > order {
            current_order -= 1;
            let buddy_ptr = current_ptr + (1 << current_order);
            unsafe { self.push_block(buddy_ptr as *mut u8, current_order) };
        }

        page
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = max(layout.size(), size_of::<FreeBlock>());
        let size = max(size, layout.align());
        let mut order = Self::order_for_size(size);

        if order > MAX_ORDER {
            return;
        }

        let mut current_ptr = ptr as usize;

        while order < MAX_ORDER {
            let buddy_addr = current_ptr ^ (1 << order);
            let buddy_ptr = buddy_addr as *mut u8;

            if unsafe { self.remove_block(buddy_ptr, order) } {
                current_ptr = min(current_ptr, buddy_addr);
                order += 1;
            } else {
                break;
            }
        }

        if order == MAX_ORDER {
            unsafe { PhysFrame::from(PhysAddr::from(current_ptr)) };
        } else {
            unsafe { self.push_block(current_ptr as *mut u8, order) };
        }
    }
}
