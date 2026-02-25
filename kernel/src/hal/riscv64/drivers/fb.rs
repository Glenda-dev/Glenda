use super::{FB, FbDriver};
use crate::drivers::fb::FramebufferWriter;

pub fn init() {
    if let Some(info) = crate::boot::get_framebuffer() {
        let writer = FramebufferWriter::new(info);
        writer.clear();
        FB.call_once(|| FbDriver::Framebuffer(writer));
    }
}
