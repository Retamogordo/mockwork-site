
#[macro_export]
macro_rules! global_usize_id {
	($id_name: ident) => {
		{
			static $id_name: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
			$id_name.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
		
		}.into()
	}
}

pub struct TakeIter<T> (Option<T>);

impl<T> TakeIter<T> {
	pub fn new(val: T) -> Self {
		Self(Some(val))
	}
} 

impl<T> Iterator for TakeIter<T> {
	type Item = T;

	fn next(&mut self) -> Option<<Self as Iterator>::Item> { 
		self.0.take()
	}
}

pub struct NoneIter<T> (std::marker::PhantomData<T>);

impl<T> Iterator for NoneIter<T> {
	type Item = T;

	fn next(&mut self) -> Option<<Self as Iterator>::Item> { 
		None
	}
}


#[macro_export]
macro_rules! warn_on_error {
	($result: expr) => {
		match $result {
			Ok(_) => (),
			Err(err) => log::warn!("{}", err),
		}
	}
}

