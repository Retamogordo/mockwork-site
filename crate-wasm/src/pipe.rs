use std::fmt::Display;
use std::collections::VecDeque;

pub trait EventTarget<T: Item + Display>: Send + Sync {
	fn notify(&mut self, curr_items: &mut SharedPipeAlloc<T>);
}
pub enum PropagationDirecion {
	Up,
	Down,
	None,
}

pub trait Item {
	fn dir(&self) -> PropagationDirecion;
}

pub struct Pipe<T: Item + Display> {
	queue: Vec<Box<dyn EventTarget<T>>>,
	shared_alloc: SharedPipeAlloc<T>,
}

impl<T: Item + Display> Pipe<T> {
	pub fn new() -> Self {
		Self { 
			queue: Vec::new(), 
			
			shared_alloc: SharedPipeAlloc { 
				items_indexed: VecDeque::with_capacity(100),
				taken_item_index: 0,
				bottom: 0,
				top_items: VecDeque::with_capacity(100),
				bottom_items: VecDeque::with_capacity(100),	
			},
		}
	}

	pub fn hook(&mut self, target: impl EventTarget<T> + 'static ) {
		self.queue.push(Box::new(target));
		self.shared_alloc.bottom += 1;
	}

	
	pub fn send(&mut self, items: impl Iterator<Item = (T, bool)>) 
		-> (&mut VecDeque<(T, bool)>, &mut VecDeque<(T, bool)>) {
		self.shared_alloc.top_items.clear();
		self.shared_alloc.bottom_items.clear();

		for item in items {
			let is_done = item.1;

			match item.0.dir() {
				PropagationDirecion::Up => {
					if self.queue.len() > 0 && !is_done {
						self.propagate((item.0, self.queue.len() - 1));
					} else {
						self.shared_alloc.top_items.push_back(item);
					} 
				},
				PropagationDirecion::Down => {
					if self.queue.len() > 0 && !is_done {
						self.propagate((item.0, 0));
					} else {
						self.shared_alloc.bottom_items.push_back(item);
					} 
				}
				PropagationDirecion::None => (),
			}
		}
		(&mut self.shared_alloc.bottom_items, &mut self.shared_alloc.top_items)
	}

	fn propagate(&mut self, item_indexed: (T, usize)) {

		self.shared_alloc.init(item_indexed);

//		while let Some(item) = self.shared_alloc.pop() {
		let mut prev_index;
		while let Some((_, index)) = self.shared_alloc.items_indexed.front() {
			let observer = &mut self.queue[*index];
			prev_index = *index;
			
			observer.notify(&mut self.shared_alloc);

			if let Some((_, index)) = self.shared_alloc.items_indexed.front() {
				if prev_index == *index {
					self.shared_alloc.propagate_current();
				}
			}
		}
	}

}

pub struct SharedPipeAlloc<T: Item + Display> {
	pub items_indexed: VecDeque<(T, usize)>,
	taken_item_index: usize,
	bottom: usize,

	top_items: VecDeque<(T, bool)>,
	bottom_items: VecDeque<(T, bool)>,
}

impl<T: Item + Display> SharedPipeAlloc<T> {
	fn init(&mut self, item: (T, usize)) {
		self.items_indexed.clear();
//		self.curr_index = item.1;
		self.items_indexed.push_back(item);
	}

	pub fn current(&self) -> &T {
		&self.items_indexed.front().as_ref().unwrap().0
	}

	pub fn take_current(&mut self) -> Option<T> {
		self.items_indexed.pop_front()
			.and_then(|item| {
				self.taken_item_index = item.1;
				Some(item.0)
			})
	}

	pub fn replace_propagate_current(&mut self, item: T) {
		self.replace_propagate_current_inner(Some(item));
	}

	pub fn propagate_current(&mut self) {
		self.replace_propagate_current_inner(Option::<T>::None);
	}

	fn replace_propagate_current_inner(&mut self, item: Option<T>) {
		if let Some(ref mut front) = self.items_indexed.front_mut() {
			if let Some(item) = item {
				front.0 = item;
			}
			match front.0.dir() {
				PropagationDirecion::Up => {
					if front.1 > 0 {
						front.1 -= 1;
					}
					else {
						self.top_items.push_back(
							(self.items_indexed.pop_front().unwrap().0, false));
					}
				},
				PropagationDirecion::Down => {
					if front.1 < self.bottom - 1 {
						front.1 += 1;
					}
					else {
						self.bottom_items.push_back(
							(self.items_indexed.pop_front().unwrap().0, false));
					}
				},
				PropagationDirecion::None => (),
			}
		}
	}

	pub fn push(&mut self, item: T) {
		let curr_index;
		if let Some(ref front) = self.items_indexed.front() {
			curr_index = front.1;
		} else {
			curr_index = self.taken_item_index;
		}

		match item.dir() {
			PropagationDirecion::Up => {
				if curr_index > 0 {
					self.items_indexed.push_back((item, curr_index - 1));
				}
				else {
					self.top_items.push_back((item, false));
				}
			},
			PropagationDirecion::Down => {
				if curr_index < self.bottom - 1 {
					self.items_indexed.push_back((item, curr_index + 1));
				}
				else {
					self.bottom_items.push_back((item, false));
				}
			},
			PropagationDirecion::None => (),
		}
	}

	pub fn push_done(&mut self, item: T) {
		match item.dir() {
			PropagationDirecion::Up => {
				self.top_items.push_back((item, true));
			},
			PropagationDirecion::Down => {
				self.bottom_items.push_back((item, true));
			},
			PropagationDirecion::None => (),
		}

	}

	pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
		for item in iter {
			self.push(item);
		}
	}
}