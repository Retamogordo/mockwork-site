
pub mod node;
pub mod physical_layer;
pub mod first_layer;
pub mod second_layer;
//mod line_stats;
pub mod line;
pub mod layer_pipe_item;
pub mod protocol_layers; // TODO should be private
mod channel_monitor;
//mod service_request;
mod address_map_resolver;
//mod address_map_resolver_fsm;
mod expired_streams_cleaner;
mod peer_channel;

use core::time::{Duration};
use instant::Instant;
use std::fmt::Display;
use std::mem;
//use colored::*;

//use crate::utils::{SeqId};
use super::protocol_stack::node::{NodeAddress};

//static LAN_MAX_DELAY: Duration = Duration::from_millis(400); 

#[derive(PartialEq, Clone)]
pub struct TimerOwnerToken {
	//	use time_stamp as unique id for PartialEq
	time_stamp: Instant,
	delay: Duration,
}
impl TimerOwnerToken {
	fn new(delay: Duration) -> Self {
//		Self { time_stamp: SystemTime::now(), delay }
		Self { time_stamp: Instant::now(), delay }
	}
	pub fn delay(&self) -> Duration {
		self.delay
	}
	pub fn created_at(&self) -> Instant {
		self.time_stamp
	}
}

#[derive(Clone, Copy, PartialEq)]   
enum Headers {
	FirstLayer(FirstLayerHeader),
	SecondLayer(SecondLayerHeader),
	NoHeader,
}

impl Headers {
	 fn stringify(&self) -> String {
		match self {
			FirstLayer(header) => header.stringify(),
			SecondLayer(header) => header.stringify(),
			NoHeader => String::from("NO HEADER "),
		}
	}

	 fn take(&mut self) -> Self {
		mem::replace(self, NoHeader)
	}

	 fn is_none(&self) -> bool {
		match *self {
			NoHeader => true,
			_ => false,
		}
	}
}

use Headers::*;

pub type MessageBodyType = MessageBody;
pub type ServiceToken = ChannelToken; // TODO should be private

#[derive(Clone)]
pub enum MessageBody {
	Data(String),
	TimeStamp(instant::Instant),
}

impl Default for MessageBody {
	fn default() -> Self {
		Self::Data("".to_string())
	}
}

impl Display for MessageBody {

	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			MessageBody::Data(msg) => write!(f, "{}", msg),
			MessageBody::TimeStamp(_ts) => {
		//				let t = ts.duration_since(SystemTime::UNIX_EPOCH).unwrap()
		//							.as_millis() % 1000000;

				write!(f, "@{}", "not implemented")
			}
		}
	}
} 	


#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelType {
	Data,
	Service,
}

#[derive(Clone, Copy, Eq)]
pub struct ChannelToken {
	id: usize,
	#[allow(dead_code)]
	channel_type: ChannelType,
	starts: Instant,
	expires: Option<Duration>,
}

impl ChannelToken {
	pub fn durable(duration: Duration, channel_type: ChannelType) -> Self {
		Self {
			id: Self::get_static_id(),
			channel_type,
			starts: Instant::now(),
			expires: Some(duration),
		}
	}

	pub fn ageless() -> Self {
		Self {
			id: Self::get_static_id(),
			channel_type: ChannelType::Service,
			starts: Instant::now(),
			expires: None,
		}
	}

	fn get_static_id() -> usize {
		crate::global_usize_id!(CHANNEL_TOKEN_ID)
	}

	pub fn id(&self) -> usize {
		self.id
	}

//	pub fn created_at(&self) -> Instant {
//		self.starts
//	}
/*	pub fn elapsed(&self) -> Duration {
		self.starts.elapsed()
	}
*/
	pub fn expired(&self) -> bool {
		if let Some(expires) = self.expires {
			self.starts.elapsed() > expires
		} else { false }
	}
	pub fn touch(&mut self) -> bool {
		if !self.expired() {
			self.starts = Instant::now();
			true
		} else { false }
	}
	pub fn expires(&self) -> Option<Duration> {
		self.expires
	}
}

impl PartialEq for ChannelToken {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
		// && self.expires == other.expires
	}
}

impl core::hash::Hash for ChannelToken {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Display for ChannelToken {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "°{}", self.id)
	}
}

impl Default for ChannelToken {
	fn default() -> Self {
		Self {
			id: crate::global_usize_id!(CHANNEL_TOKEN_ID),
			channel_type: ChannelType::Data,
		//	starts: Option<SystemTime>,
			starts: Instant::now(),
			expires: Some(Duration::from_secs(0)),		
		}
	}
}

#[derive(Clone, Copy)]   
pub enum ChunkType {
	ChannelRequest(bool),
	ChannelResponse,
	ChannelDrop,
	ChannelKeepAlive,
	Data,
	Service,
}
//use ChunkType::{*};

#[derive(Clone, Copy)]   
pub struct FirstLayerHeader {
	id: usize,
//	timestamp: RequestResponseTimestamp, 
	//timestamp: SystemTime, 
//	timestamp: Instant, 
	chunk_type: ChunkType,
	channel_token: Option<ChannelToken>,
}

impl FirstLayerHeader {
	fn new(
//		timestamp: RequestResponseTimestamp, 
//		timestamp: SystemTime, 
//		timestamp: Instant, 
		chunk_type: ChunkType,
		channel_token: Option<ChannelToken>,
	) -> Self {
		Self {
 			id: crate::global_usize_id!(CHUNK_ID),
//			timestamp, 
			chunk_type,
			channel_token,
		}
	}
	fn stringify(&self) -> String {
		let t = " "; //timestamp to be fixed";
		let tk = if self.channel_token.is_none() {format!("none")} else 
			{format!("{}", self.channel_token.unwrap())};
//		let t = self.timestamp
//		.duration_since(SystemTime::UNIX_EPOCH).unwrap()
//		.as_millis() % 1000000;
		
		match self.chunk_type {
//			Request => format!("[req,#{},{} ", self.id, self.timestamp),
//			Response => format!("[resp,#{},{} ", self.id, self.timestamp),
//			ChunkType::Cast => format!("[cast,#{},{} ", self.id, self.timestamp),
			ChunkType::ChannelRequest(_) => format!("[ch-req,#{},{}{} ", self.id, t, tk),
			ChunkType::ChannelResponse => format!("[ch-resp,#{},{}{} ", self.id, t, tk),
			ChunkType::ChannelDrop => format!("[ch-drop,#{},{}{} ", self.id, t, tk),
			ChunkType::ChannelKeepAlive => format!("[ch-ack,#{},{}{} ", self.id, t, tk),
			ChunkType::Data => format!("[data,#{},{}{} ", self.id, t, tk),
			ChunkType::Service => format!("[srvc,#{},{}{} ", self.id, t, tk),
		}	
	}
}

impl Into<Headers> for FirstLayerHeader {
	fn into(self) -> Headers {
		Headers::FirstLayer(self)
	}
}
impl<'a> From<&'a Headers> for &'a FirstLayerHeader {
	fn from(h: &'a Headers) -> Self {
		match h {
			FirstLayer(fh) => fh,
			_ => panic!("Cannot convert header to First Layer Header")
		}
	}
}

impl PartialEq for FirstLayerHeader {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
//		self.id == other.id && self.timestamp == other.timestamp
	}
}

impl Display for FirstLayerHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}", self.stringify())
	}
}

#[derive(Clone, Copy, PartialEq)]   
pub enum BlockType {
	AddressMapRequest,
	AddressMapResponse,
	ChannelRequest,
	DropToken,
//	ChannelResponse,
//	KeepAliveRequest,
	Data,
//	ChannelExpirationCheck,
	Unspecified,
}

#[derive(Clone, Copy)]   
pub struct SecondLayerHeader {
	block_type: BlockType,
	src: NodeAddress,
	dest:NodeAddress,
//	service_token: Option<ServiceToken>,
}

impl SecondLayerHeader {
	pub fn new(block_type: BlockType, src: NodeAddress, dest: NodeAddress, 
//		service_token: Option<ServiceToken>
	) -> Self {
		Self { 
			block_type,
			src,
			dest,
//			service_token,
		 }
	}

	 fn stringify(&self) -> String {
		format!("{}{}->{}",
			match self.block_type {
				BlockType::AddressMapRequest => format!("[addr-map-req "),
				BlockType::AddressMapResponse => format!("[addr-map-resp "),
				BlockType::ChannelRequest => format!("[req chnl "),
//				BlockType::ChannelResponse => format!("[resp chnl "),
//				BlockType::KeepAliveRequest => format!("[keep alive "),
				BlockType::Data => format!("[data "),
		//		BlockType::Unspecified 
			 	_ => format!("[? "),
			}, self.src, self.dest
		)
	}
}

impl Into<Headers> for SecondLayerHeader {
	fn into(self) -> Headers {
		Headers::SecondLayer(self)
	}
}

impl<'a> From<&'a Headers> for &'a SecondLayerHeader {
	fn from(h: &'a Headers) -> Self {
		match h {
			SecondLayer(sh) => sh,
			_ => panic!("Cannot convert header into Second Layer Header")
		}
	}
}

impl PartialEq for SecondLayerHeader {
	fn eq(&self, _other: &Self) -> bool {
		unimplemented!();
	}
}

pub struct Chunk {
	headers: [Headers; 2],
	body: MessageBodyType,
}

impl Chunk {
	pub fn from_message(message: MessageBodyType) -> Self {
		Self { 
			body: message, 
			headers: [NoHeader, NoHeader], 
		}
	}
/*
	fn empty() -> Self {
		Self::from_message("".to_string())
	}
*/
	fn swaddle<H: Into<Headers>>(&mut self, header: H) {
		let hdr: Headers = header.into();
		let index: usize;
		match hdr {
			FirstLayer(_) => index = 0,
			SecondLayer(_) => index = 1,
			NoHeader => return,
		}
		self.headers[index] = hdr;
	}
	fn unswaddle(&mut self) -> Headers {
		if let Some(index) = self.headers.iter().position(|h| !h.is_none()) {
			self.headers[index].take()
		} else { NoHeader }
	}

	fn header(&self) -> &Headers {
		if let Some(index) = self.headers.iter().position(|h| !h.is_none()) {
			&self.headers[index]
		} else { &NoHeader }
	}

	pub fn body(&self) -> &MessageBodyType {
		&self.body
	}
	pub fn take(self) -> MessageBodyType {
		self.body
	}

}

impl Default for Chunk {
	fn default() -> Self {
		Self::from_message(MessageBodyType::default())
	}
}

impl Clone for Chunk {
	fn clone(&self) -> Self {
		Self { 
			body: self.body.clone(), 
			headers: [self.headers[0], self.headers[1]], 
		}
	}
}

impl Display for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		let mut s = String::from("");
		let iter = self.headers.iter().rev();
		for header in iter {
			s = format!("{}{}", s, header.stringify());
		}
		write!(f, "{}{}", s, self.body)
	}
}
