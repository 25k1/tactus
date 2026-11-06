use std::ffi::c_int;
use tactus_winamp::Winamp;
use tactus_winamp::sdk::{InModule, OutModule};

macro_rules! call {
    ($function:expr, $fallback:expr $(, $argument:expr)*) => {
        match $function {
            Some(function) => unsafe { function($($argument),*) },
            None => $fallback,
        }
    };
}

#[derive(Clone, Copy)]
pub struct Host(*mut InModule);

unsafe impl Send for Host {}

impl Host {
    pub fn get() -> Self {
        Self(crate::MODULE.get())
    }

    fn module(&self) -> &InModule {
        unsafe { &*self.0 }
    }

    pub fn winamp(&self) -> Winamp {
        Winamp::new(self.module().main_window)
    }

    pub fn output(&self) -> Option<Output> {
        let output = self.module().out;
        (!output.is_null()).then_some(Output(output))
    }

    pub fn visual_init(&self, latency: c_int, rate: c_int) {
        call!(self.module().sa_vsa_init, (), latency, rate);
    }

    pub fn visual_deinit(&self) {
        call!(self.module().sa_vsa_deinit, ());
    }

    pub fn visual_info(&self, rate: c_int, channels: c_int) {
        call!(self.module().vsa_set_info, (), rate, channels);
    }

    pub fn info(&self, kilobits: c_int, kilohertz: c_int, stereo: bool) {
        call!(self.module().set_info, (), kilobits, kilohertz, c_int::from(stereo), 1);
    }

    pub fn add_pcm(&self, samples: &mut [i16], channels: usize, time: c_int) {
        let module = self.module();
        let (data, channels) = (samples.as_mut_ptr().cast(), channels as c_int);
        call!(module.sa_add_pcm, (), data, channels, 16, time);
        call!(module.vsa_add_pcm, (), data, channels, 16, time);
    }

    pub fn dsp_active(&self) -> bool {
        call!(self.module().dsp_is_active, 0) != 0
    }

    pub fn process(&self, samples: &mut [i16], frames: usize, channels: usize, rate: u32) -> usize {
        if !self.dsp_active() {
            return frames;
        }
        let (data, count) = (samples.as_mut_ptr(), frames as c_int);
        let produced = call!(self.module().dsp_do_samples, count, data, count, 16, channels as c_int, rate as c_int);
        (produced.max(0) as usize).min(samples.len() / channels)
    }
}

#[derive(Clone, Copy)]
pub struct Output(*mut OutModule);

unsafe impl Send for Output {}

impl Output {
    fn module(&self) -> &OutModule {
        unsafe { &*self.0 }
    }

    pub fn open(&self, rate: c_int, channels: c_int) -> c_int {
        call!(self.module().open, -1, rate, channels, 16, -1, -1)
    }

    pub fn close(&self) {
        call!(self.module().close, ());
    }

    pub fn write(&self, samples: &[i16]) {
        call!(self.module().write, 0, samples.as_ptr().cast_mut().cast(), (samples.len() * 2) as c_int);
    }

    pub fn can_write(&self) -> usize {
        call!(self.module().can_write, 0).max(0) as usize
    }

    pub fn is_playing(&self) -> bool {
        call!(self.module().is_playing, 0) != 0
    }

    pub fn pause(&self, paused: bool) {
        call!(self.module().pause, 0, c_int::from(paused));
    }

    pub fn set_volume(&self, volume: c_int) {
        call!(self.module().set_volume, (), volume);
    }

    pub fn set_pan(&self, pan: c_int) {
        call!(self.module().set_pan, (), pan);
    }

    pub fn flush(&self, time: c_int) {
        call!(self.module().flush, (), time);
    }

    pub fn output_time(&self) -> c_int {
        call!(self.module().output_time, 0)
    }

    pub fn written_time(&self) -> c_int {
        call!(self.module().written_time, 0)
    }
}
