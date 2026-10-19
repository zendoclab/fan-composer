use fan_composer::synth::{Mode, Parameters, Synth};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {static TRACK:Cell<bool>=const{Cell::new(false)};static ALLOCATIONS:Cell<usize>=const{Cell::new(0)};}
struct MeasuredAllocator;
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;
#[test]
fn callback_dsp_and_parameter_transport_do_not_allocate_after_setup() {
    let mut synth = Synth::new(48000.0, 255);
    let (mut tx, mut rx) = rtrb::RingBuffer::new(16);
    let (mut reference_tx, mut reference_rx) = rtrb::RingBuffer::new(128);
    let mut checksum = 0.0;
    TRACK.with(|v| v.set(true));
    for i in 0..48000 * 3 {
        if i % 4800 == 0 {
            tx.push(Parameters {
                mode: Mode::ALL[(i / 4800) % 3],
                ..Default::default()
            })
            .unwrap();
            synth.set_parameters(rx.pop().unwrap());
        }
        let output = synth.next_frame(0.04, 0.06);
        reference_tx
            .push(fan_composer::auto_volume::ReferenceFrame { channels: output })
            .unwrap();
        checksum += reference_rx.pop().unwrap().channels[0];
    }
    TRACK.with(|v| v.set(false));
    assert!(checksum.is_finite());
    assert_eq!(
        ALLOCATIONS.with(Cell::get),
        0,
        "real-time DSP allocated memory"
    );
}
