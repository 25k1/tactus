use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Capabilities(u32);

impl Capabilities {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u32::MAX);
    pub const WAVE: Self = Self(1);
    pub const GENERATED: Self = Self(1 << 1);
    pub const DISLIKE: Self = Self(1 << 2);
    pub const PINS: Self = Self(1 << 3);
    pub const UPLOAD: Self = Self(1 << 4);
    pub const DOWNLOAD: Self = Self(1 << 5);
    pub const LYRICS: Self = Self(1 << 6);
    pub const PRESAVE: Self = Self(1 << 7);
    pub const CONCERTS: Self = Self(1 << 8);
    pub const COLLECTIVE: Self = Self(1 << 9);
    pub const TRAILER: Self = Self(1 << 10);
    pub const ACCOUNT_SETTINGS: Self = Self(1 << 11);
    pub const PROMO_CODES: Self = Self(1 << 12);
    pub const REMOTE_QUEUE: Self = Self(1 << 13);
    pub const YNISON: Self = Self(1 << 14);
    pub const CONNECT: Self = Self(1 << 15);
    pub const CHARTS: Self = Self(1 << 16);
    pub const PODCASTS: Self = Self(1 << 17);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for Capabilities {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

impl BitOrAssign for Capabilities {
    fn bitor_assign(&mut self, other: Self) {
        *self = self.union(other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_and_checks_flags() {
        let set = Capabilities::WAVE | Capabilities::LYRICS;
        assert!(set.contains(Capabilities::WAVE));
        assert!(set.contains(Capabilities::WAVE | Capabilities::LYRICS));
        assert!(!set.contains(Capabilities::YNISON));
        assert!(set.contains(Capabilities::NONE));
    }
}
