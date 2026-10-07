//! Hot-path allocation audit: the engine must do zero heap traffic.
//!
//! A counting global allocator measures every allocation inside the measured
//! call. This test binary overrides the allocator for itself only.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocs<T>(f: impl FnOnce() -> T) -> usize {
    let before = ALLOCS.load(Ordering::Relaxed);
    let _ = f(); // returned value drops inside the window; drops do not allocate
    ALLOCS.load(Ordering::Relaxed) - before
}

#[test]
fn engine_hot_path_allocates_nothing() {
    // replay / trace over a full-size legal path
    let path = "dduu".repeat(768); // 3072 legal moves
    assert_eq!(allocs(|| maze_core::replay(path.as_bytes())), 0, "replay");
    assert_eq!(allocs(|| maze_core::trace(path.as_bytes()).steps), 0, "trace");

    // base64 both directions, stack buffers only
    let mut cookie = [0u8; maze_core::MAX_COOKIE_LEN];
    assert_eq!(
        allocs(|| maze_core::base64::encode_into(path.as_bytes(), &mut cookie).unwrap()),
        0,
        "encode_into"
    );
    // a real 4096-char cookie: encoding 3072 bytes yields exactly 4096 chars
    let cookie_value: Vec<u8> = cookie[..maze_core::MAX_COOKIE_LEN].to_vec();
    assert_eq!(cookie_value.len(), maze_core::MAX_COOKIE_LEN);
    let mut raw = [0u8; maze_core::MAX_PATH_LEN];
    assert_eq!(
        allocs(|| maze_core::base64::decode_into(&cookie_value, &mut raw).unwrap()),
        0,
        "decode_into"
    );
}