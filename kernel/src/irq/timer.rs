use crate::hal;

pub const TIME_SLICE_MS: usize = 100;

pub fn program_next_tick() {
    let next = hal::timer::get_time() + TIME_SLICE_MS;
    hal::timer::set_next_event(next);
}
