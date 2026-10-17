use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const WARNING: Duration = Duration::from_secs(60);
pub const FADE: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SleepTimer {
    Minutes(u32),
    EndOfTrack,
    EndOfQueue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    Warn,
    FadeOut,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sleep {
    pub timer: SleepTimer,
    deadline: Option<Instant>,
    warned: bool,
    stop_at: Option<Instant>,
}

impl Sleep {
    pub fn new(timer: SleepTimer, now: Instant) -> Self {
        let deadline = match timer {
            SleepTimer::Minutes(minutes) => Some(now + Duration::from_secs(u64::from(minutes) * 60)),
            SleepTimer::EndOfTrack | SleepTimer::EndOfQueue => None,
        };
        let warned = deadline.is_none_or(|deadline| deadline <= now + WARNING);
        Self { timer, deadline, warned, stop_at: None }
    }

    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline.map(|deadline| deadline.saturating_duration_since(now))
    }

    pub fn step(&mut self, now: Instant) -> Step {
        if let Some(stop_at) = self.stop_at {
            return if now >= stop_at { Step::Stop } else { Step::Wait };
        }
        let Some(deadline) = self.deadline else {
            return Step::Wait;
        };
        if now >= deadline {
            self.stop_at = Some(now + FADE);
            return Step::FadeOut;
        }
        if !self.warned && now + WARNING >= deadline {
            self.warned = true;
            return Step::Warn;
        }
        Step::Wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_sleep_warns_fades_and_stops() {
        let start = Instant::now();
        let mut sleep = Sleep::new(SleepTimer::Minutes(15), start);
        assert_eq!(sleep.remaining(start), Some(Duration::from_secs(900)));
        assert_eq!(sleep.step(start + Duration::from_secs(60)), Step::Wait);
        assert_eq!(sleep.step(start + Duration::from_secs(845)), Step::Warn);
        assert_eq!(sleep.step(start + Duration::from_secs(846)), Step::Wait);
        let deadline = start + Duration::from_secs(900);
        assert_eq!(sleep.step(deadline), Step::FadeOut);
        assert_eq!(sleep.step(deadline + Duration::from_secs(5)), Step::Wait);
        assert_eq!(sleep.step(deadline + FADE), Step::Stop);
    }

    #[test]
    fn short_timers_skip_the_warning() {
        let start = Instant::now();
        let mut sleep = Sleep::new(SleepTimer::Minutes(1), start);
        assert_eq!(sleep.step(start + Duration::from_secs(30)), Step::Wait);
        assert_eq!(sleep.step(start + Duration::from_secs(60)), Step::FadeOut);
    }

    #[test]
    fn end_of_track_has_no_deadline() {
        let start = Instant::now();
        let mut sleep = Sleep::new(SleepTimer::EndOfTrack, start);
        assert_eq!(sleep.remaining(start), None);
        assert_eq!(sleep.step(start + Duration::from_secs(10_000)), Step::Wait);
    }
}
