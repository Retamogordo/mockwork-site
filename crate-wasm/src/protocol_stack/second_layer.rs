use core::time::{Duration};
use strum_macros::{Display};
use super::first_layer::{*};
use super::{TimerOwnerToken, 
	Chunk, ChunkType,
	FirstLayerHeader, SecondLayerHeader, ChannelToken, ServiceToken};
use super::node::{NodeAddress};
use crate::protocol_stack::layer_pipe_item::{*};

#[derive(Clone, Display)]
pub enum ServiceRequestType {
	LineChannel(bool),
	LineStats(usize),
	AddressMap(NodeAddress),
	PeerChannel(NodeAddress),
}

#[derive(Clone, Display)]
pub enum ServiceResultType {
	LineChannel(LineChannelResult),
	LineStats(LineStatsResult),
	AddressMapUpdate(NodeAddress),
	SomeService(Chunk),
}

impl ServiceResultType {
	pub fn data(&self) -> Option<&Chunk> {
		match self {
			LineChannel(res) => res.data(),
			_ => Option::<&Chunk>::None,
		}
	}
}

#[derive(Clone, Display)]
pub enum LineChannelResult {
//	Established,
	Data(Chunk),
	Expired,
	Dropped,
	TooManyChannels(usize),
//	InvalidToken,
	Disconnected,
	LineNotAvailable,
	NeverActivated,
	NodeNotFound,
//	ServiceComplete,
	Unspecified,
	None
}

impl LineChannelResult {
	pub fn data(&self) -> Option<&Chunk> {
		match self {
			Data(data) => Some(data),
			_ => Option::<&Chunk>::None,
		}
	}
}

#[derive(Clone, Display)]
pub enum LineStatsResult {
	Ready(LineStatistics),
	Failure,
	LineNotAvailable,
}

#[derive(Clone, Display)]
pub enum SecondLayerEvent {
	BlockReceived(Chunk),
	OnSocketShutdown,
	OnSocketConnected,

	ServiceStarted(ServiceToken),
	ServiceMessage(ServiceToken, ServiceResultType),
	ServiceComplete(ServiceToken, ServiceResultType),
	
	IncomingDataChannel(ChannelToken),
	IncomingPeerChannel(NodeAddress, ServiceToken),
	FirstLayerChannelKeptAlive(ChannelToken),
	
	TimerFinished(TimerOwnerToken),

	None,
}

impl Default for SecondLayerEvent {
	fn default() -> Self {
		Self::None
	}
}

#[derive(Clone, Display)]
pub enum SecondLayerCommand {	
    StartTimer(TimerOwnerToken),
	Send(Chunk, SecondLayerHeader, Option<ChannelToken>),
	SendAsIs(Chunk, Option<ChannelToken>),

	ServiceRequest(ServiceToken, ServiceRequestType),
	DropService(ServiceToken),
	PollService(ServiceToken),

	StartExpiredStreamsCleaner(Duration),
	StopExpiredStreamsCleaner,

	CheckExpiredTokens,
	PollTraffic,

	None,
}

impl SecondLayerCommand {
	pub fn wrap(self) -> LayerCommand {
		LayerCommand::SecondLayer(self)
	}
	pub fn take(&mut self) -> Self {
		std::mem::replace(self, Self::None)
	}
}

use ServiceResultType::{*};
use LineChannelResult::{*};
use LineStatsResult::{*};

impl From<FirstLayerEvent> for SecondLayerEvent {
	fn from(event: FirstLayerEvent) -> Self {
		match event {
			FirstLayerEvent::ChunkReceived(mut chunk, _at) => {
				let hdr = chunk.unswaddle(); 
				let hdr: &FirstLayerHeader = (&hdr).into();
				
				match hdr.channel_token {
					Some(token) => {
						match hdr.chunk_type {
							ChunkType::Data => 
								Self::ServiceMessage(token, LineChannel(Data(chunk))),
							ChunkType::Service =>
								Self::ServiceMessage(token, SomeService(chunk)),
							_ => Self::BlockReceived(chunk),
						}
					}
					Option::<ChannelToken>::None => Self::BlockReceived(chunk),
				}
			},
			FirstLayerEvent::TimerFinished(token) => Self::TimerFinished(token),
			FirstLayerEvent::ChannelEstablished(channel_token) 
				=> Self::ServiceStarted(channel_token),
			FirstLayerEvent::IncomingDataChannel(channel_token) 
				=> Self::IncomingDataChannel(channel_token),
			
			FirstLayerEvent::ChannelExpired(channel_token) =>
				Self::ServiceComplete(channel_token, LineChannel(Expired)),
			FirstLayerEvent::ChannelKeptAlive(channel_token) =>
				Self::FirstLayerChannelKeptAlive(channel_token),
						
			FirstLayerEvent::ChannelDropped(channel_token) =>
				Self::ServiceComplete(channel_token, LineChannel(Dropped)),
		
			FirstLayerEvent::TooManyChannels(service_token, n) 
				=> Self::ServiceComplete(service_token, LineChannel(TooManyChannels(n))),

			FirstLayerEvent::HandshakeFailure(channel_token)
				=> Self::ServiceComplete(channel_token, LineChannel(LineChannelResult::LineNotAvailable)),
			FirstLayerEvent::LineStatsStarted(service_token) 
				=> Self::ServiceStarted(service_token), 

			FirstLayerEvent::LineStatsReady(service_token, stats) 
				=> Self::ServiceComplete(service_token, LineStats(Ready(stats))),

			FirstLayerEvent::LineStatsFailure(service_token)
				=> Self::ServiceComplete(service_token, LineStats(Failure)),

			FirstLayerEvent::None => Self::None,
			_ => { log::error!("Not implemented for {}", event);
				Self::None
			},
		}
	}
}

impl SecondLayerEvent {
	pub fn wrap(self) -> LayerEvent {
		LayerEvent::SecondLayer(self)
	}
	pub fn take(&mut self) -> Self {
		std::mem::replace(self, Self::None)
	}
}
