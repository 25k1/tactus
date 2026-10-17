use sonic_rs_sys::{
    sonicCreateStream, sonicDestroyStream, sonicFlushStream, sonicReadFloatFromStream, sonicSamplesAvailable,
    sonicSetSpeed, sonicStreamStruct, sonicWriteFloatToStream,
};
use std::ptr::NonNull;

pub struct Sonic {
    stream: NonNull<sonicStreamStruct>,
    channels: usize,
}

unsafe impl Send for Sonic {}

impl Sonic {
    pub fn new(rate: u32, channels: u16) -> Option<Self> {
        let stream = unsafe { sonicCreateStream(i32::try_from(rate).ok()?, i32::from(channels)) };
        NonNull::new(stream).map(|stream| Self { stream, channels: usize::from(channels.max(1)) })
    }

    pub fn set_speed(&mut self, speed: f32) {
        unsafe { sonicSetSpeed(self.stream.as_ptr(), speed) };
    }

    pub fn write(&mut self, samples: &[f32]) {
        let frames = i32::try_from(samples.len() / self.channels).unwrap_or(i32::MAX);
        if frames > 0 {
            unsafe { sonicWriteFloatToStream(self.stream.as_ptr(), samples.as_ptr(), frames) };
        }
    }

    pub fn flush(&mut self) {
        unsafe { sonicFlushStream(self.stream.as_ptr()) };
    }

    pub fn read(&mut self, output: &mut Vec<f32>) -> bool {
        let available = unsafe { sonicSamplesAvailable(self.stream.as_ptr()) }.max(0);
        output.clear();
        if available == 0 {
            return false;
        }
        output.resize(available as usize * self.channels, 0.0);
        let read = unsafe { sonicReadFloatFromStream(self.stream.as_ptr(), output.as_mut_ptr(), available) };
        output.truncate(read.clamp(0, available) as usize * self.channels);
        !output.is_empty()
    }
}

impl Drop for Sonic {
    fn drop(&mut self) {
        unsafe { sonicDestroyStream(self.stream.as_ptr()) };
    }
}
