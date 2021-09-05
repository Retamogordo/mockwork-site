use std::fmt::Display;
use core::time::{Duration};
use rand::random;
use instant::Instant;
use serde::{Serialize};

#[derive(Clone, Copy, Eq, Hash, Debug)]
pub struct LineId(usize);

impl LineId {
    pub fn new() -> Self {
		crate::global_usize_id!(LINE_ID)
    }
}

impl From<usize> for LineId {
	fn from(n: usize) -> Self {
		Self(n)
	}
}

impl PartialEq for LineId {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}
impl Default for LineId {
	fn default() -> Self {
		Self(0) 
	}
}

impl Display for LineId {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl Serialize for LineId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
		let state = serializer.serialize_newtype_struct("id", &self.0)?;
		Ok(state)
    }
}

type DataBodyType = String;

#[derive(Debug)]
pub struct Payload {
	body: DataBodyType,
}

#[derive(Clone, Copy, Eq, Hash, Debug)]
pub enum OneOfTwo<T: PartialEq> {
	First(T),
	Second(T),
}

use OneOfTwo::*;

impl<T: PartialEq> OneOfTwo<T> {
	pub fn as_val_ref(&self) -> &T {
		match self {
			First(ref expr) => expr,
			Second(ref expr) => expr,
		}		
	}
}

impl<T: PartialEq> PartialEq for OneOfTwo<T> {
	fn eq(&self, other: &Self) -> bool {
		std::mem::discriminant(self) == std::mem::discriminant(other) &&
		*self.as_val_ref() == *other.as_val_ref()
	}
}

pub type LineTransceiverId = OneOfTwo<LineId>;

impl Display for LineTransceiverId {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			First(n) => write!(f, "{}(1)", n), 
			Second(n) => write!(f, "{}(2)", n),
		}
	}
}
#[derive(Clone)]
struct CurrentSleepingDelay {
	last_time_stamp: Option<Instant>,
	delay: Duration,
}

impl CurrentSleepingDelay {
	fn from_delay(delay: Duration) -> Self {
		Self { 
			last_time_stamp: None,
			delay,
		}
	}
}

#[derive(Clone)]
pub struct LineModel {
	pub propagation_delay: Duration,
	noise_ratio: f32,
	curr_propagation_sleep_delay: CurrentSleepingDelay,
}

impl LineModel {
	pub fn new(propagation_delay: Duration, 
		noise_ratio: f32
	) -> Self {
		Self {
			propagation_delay,
//			transmission_latency,
			noise_ratio,
			curr_propagation_sleep_delay: CurrentSleepingDelay::from_delay(propagation_delay),
		}
	}

	pub fn calc_receive_time(&mut self, transmit_time: Instant) -> Option<Instant> {
		if self.noise_ratio < random() {
//			let delay = self.curr_propagation_sleep_delay.next_delay(transmit_time);
			let delay = self.propagation_delay;
			let receive_time = transmit_time.checked_add(delay);
			receive_time
		} else { None }
	}

	pub fn probable_delay(&self) -> Duration {
		self.propagation_delay 
		+ self.propagation_delay/5 
//		+ self.transmission_latency
	}
}

