use core::time::{ Duration};
use instant::Instant;
use strum_macros::{Display};

use crate::pipe::{EventTarget, SharedPipeAlloc};
use super::{TimerOwnerToken};
use super::physical_layer::{*};
use super::second_layer::{*};
use super::{ChannelToken, *};
use crate::protocol_stack::layer_pipe_item::{*};

#[derive(Clone, Copy)]
pub struct LineStatistics {
	pub line_quality: f32,
	pub oneway_trip_delay: Duration,
	pub returned_chunks: usize,
}

impl LineStatistics {
	pub fn new() -> Self {
		Self { 
			line_quality: 0.0,
			oneway_trip_delay: Duration::from_millis(0),
			returned_chunks: 0,
		}
	}
}

impl std::fmt::Display for LineStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "line quality in range [0..1]: {}, one way trip delay {}, returned {} chunks", 
			self.line_quality, 
			self.oneway_trip_delay.as_millis(), 
			self.returned_chunks)
	}
}

#[derive(Clone, Display)]
pub enum FirstLayerEvent {
	ChunkReceived(Chunk, Instant),
    TimerFinished(TimerOwnerToken),
	LineOk(Chunk, Duration),
	PingLineTimeout,
	HandshakeFailure(ChannelToken),
	ChannelEstablished(ChannelToken),
	IncomingDataChannel(ChannelToken),
	ChannelExpired(ChannelToken),
	ChannelKeptAlive(ChannelToken),
	ChannelDropped(ChannelToken),
	TooManyChannels(ChannelToken, usize),
	LineStatsStarted(ServiceToken),
	LineStatsReady(ServiceToken, LineStatistics),
	LineStatsFailure(ServiceToken),
	None,
}

impl FirstLayerEvent {
	pub fn wrap(self) -> LayerEvent {
		LayerEvent::FirstLayer(self)
	}
	pub fn take(&mut self) -> Self {
		mem::replace(self, Self::None)
	}
}

#[derive(Clone, Display)]
pub enum FirstLayerCommand {
    StartTimer(TimerOwnerToken),
	PassToSend(Chunk, ChunkType, Option<ChannelToken>),
	Send(Chunk, FirstLayerHeader),
	PingLineForStats(usize, ChannelToken),
	LineStats(ServiceToken, usize),
	ChannelRequest(ChannelToken, bool),
	DropService(ServiceToken),
	CheckExpiredTokens,
	PollTraffic,
	None,
}

impl FirstLayerCommand {
	pub fn wrap(self) -> LayerCommand {
		LayerCommand::FirstLayer(self)
	}
	pub fn take(&mut self) -> Self {
		mem::replace(self, Self::None)
	}
}

pub struct DefaultChunkSender {
}

impl DefaultChunkSender {
	pub fn new() -> Self {
		Self {
		}
	}
}

impl EventTarget<LayerPipeItem> for DefaultChunkSender {

	fn notify(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
		match shared_alloc.current() {
			LayerPipeItem::SomeCommand(_, LayerCommand::FirstLayer(cmd)) =>
			
			match cmd {
				FirstLayerCommand::PassToSend(chunk, chunk_type, channel_token) => {

				log::debug!("{}", format!("\tcmd: ~~> {}", chunk));
//						shared_alloc.clear();
					let clone = chunk.clone();
					let hdr = FirstLayerHeader::new(
						chunk_type.clone(),
						channel_token.clone());

					shared_alloc.replace_propagate_current(
						(FirstLayerCommand::Send(clone, hdr)).wrap().into()
					);
				},
				_ => (), // shared_alloc.push(cmd.into()),
			},

			_ => (), //shared_alloc.push(item),
		}
	}
}
impl From<PhysicalLayerEvent> for FirstLayerEvent {
	fn from(ev: PhysicalLayerEvent) -> Self {
		match ev {
			PhysicalLayerEvent::Received(payload) => 
				Self::ChunkReceived(payload.data, 
					payload.transmit_time),
			
			PhysicalLayerEvent::TimerFinished(token) => Self::TimerFinished(token),
			PhysicalLayerEvent::PingLineTimeout => Self::PingLineTimeout,
			PhysicalLayerEvent::LineOk(chunk, trip_delay) => Self::LineOk(chunk, trip_delay),
			_ => Self::None,
		}
	}
}

impl From<SecondLayerCommand> for FirstLayerCommand {
	fn from(cmd: SecondLayerCommand) -> Self {
		match cmd {
			SecondLayerCommand::Send(mut block, header, channel_token) => {
				let chunk_type = match header.block_type {
					BlockType::Data => ChunkType::Data,
					_ => ChunkType::Service,
				};
				block.swaddle(header);
		
				Self::PassToSend(block, chunk_type, channel_token)
			},

			SecondLayerCommand::SendAsIs(block, channel_token) => {
				let hdr: &SecondLayerHeader = block.header().into();
				let chunk_type = match hdr.block_type {

					BlockType::Data => ChunkType::Data,
					_ => ChunkType::Service,
				};
				Self::PassToSend(block, chunk_type, channel_token)	
			}
			SecondLayerCommand::StartTimer(token) => Self::StartTimer(token),
				
			SecondLayerCommand::ServiceRequest(service_token, req_type) => {
				log::debug!("service request: {}, service token {}", req_type, service_token);
				match req_type {
					ServiceRequestType::LineChannel(exclusive) 
						=> Self::ChannelRequest(service_token, exclusive),
					ServiceRequestType::LineStats(batch) => Self::LineStats(service_token, batch),
					ServiceRequestType::AddressMap(_dest) => unreachable!("Must be handled by Address Map Resolver"),
					ServiceRequestType::PeerChannel(_dest) => unreachable!("Must be handled by Peer Channel Monitor"),
				}
			}, 
			SecondLayerCommand::DropService(service_token) => {
				Self::DropService(service_token)
			},
			SecondLayerCommand::PollTraffic => FirstLayerCommand::PollTraffic,
			SecondLayerCommand::CheckExpiredTokens => FirstLayerCommand::CheckExpiredTokens,
			_ => Self::None,
		}
	}
}
