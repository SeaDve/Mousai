use gtk::glib;

use std::time::Duration;

/// A boxed [`Duration`](Duration)
#[derive(Debug, Default, Clone, Copy, glib::Boxed)]
#[boxed_type(name = "MsaiClockTime", nullable)]
pub struct ClockTime(Duration);

impl ClockTime {
    pub const ZERO: Self = Self(Duration::ZERO);

    pub fn from_secs_f64(secs: f64) -> Self {
        Self(Duration::from_secs_f64(secs))
    }

    pub const fn from_secs(secs: u64) -> Self {
        Self(Duration::from_secs(secs))
    }

    pub const fn from_micros(micros: u64) -> Self {
        Self(Duration::from_micros(micros))
    }

    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    pub const fn as_secs(&self) -> u64 {
        self.0.as_secs()
    }

    pub const fn as_micros(&self) -> u128 {
        self.0.as_micros()
    }
}

impl From<gst::ClockTime> for ClockTime {
    fn from(value: gst::ClockTime) -> Self {
        Self(value.into())
    }
}

impl TryFrom<ClockTime> for gst::ClockTime {
    type Error = anyhow::Error;

    fn try_from(value: ClockTime) -> Result<Self, Self::Error> {
        gst::ClockTime::try_from(value.0).map_err(|err| err.into())
    }
}
