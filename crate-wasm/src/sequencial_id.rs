
use SequencialGenerator::{Next};
#[derive(Debug)]

pub enum SequencialGenerator<T: SequencialId> {
    Next(T),
}

impl <T: SequencialId> SequencialGenerator<T> {
	pub fn new() -> Self {
	    Next(T::first())
	}

	pub fn next(&mut self) -> &mut Self{
		*self = Next(
			match self {
				Next(ref x) => T::next(x)
			}
		);
		self
    }

	pub fn value(&self) -> T {
		match self {
			Next(x) => *x
		}
	}
}

pub trait SequencialId: Copy + PartialEq {
	fn first() -> Self;
	fn next(&self) -> Self;
}

