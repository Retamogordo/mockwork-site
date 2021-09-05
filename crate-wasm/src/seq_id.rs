use crate::sequencial_id::SequencialId;
use std::fmt::Display;

#[derive(Clone, Copy, PartialEq)]
pub struct SeqId(pub u32);

impl SeqId {
	fn last() -> Self {
		Self(u32::MAX)		
	}
}

impl SequencialId for SeqId {
	fn first() -> Self {
		Self(1)
	}

	fn next(&self) -> Self {
		Self( if self.0 < Self::last().0 {self.0 + 1} else {Self::first().0} )
	}
}

impl Display for SeqId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    	write!(f, "{}",  self.0)
	}
}

#[derive(Clone, Copy, PartialEq)]
pub struct LineId(pub u32);

impl LineId {
	pub fn any() -> Self {
		Self(u32::MAX)
	}

	fn last() -> Self {
		Self(u32::MAX - 1)		
	}
}

impl SequencialId for LineId {
	fn first() -> Self {
		Self(1)
	}

	fn next(&self) -> Self {
		Self( if self.0 < Self::last().0 {self.0 + 1} else {panic!("No more lines available to wire")} )
	}
}

impl Display for LineId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    	write!(f, "{}",  self.0)
	}
}
