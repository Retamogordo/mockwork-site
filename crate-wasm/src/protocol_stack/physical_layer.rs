use core::time::{Duration};
use instant::Instant;
use strum_macros::{Display};
use crate::pipe::{EventTarget, SharedPipeAlloc};
use super::{TimerOwnerToken};
use super::first_layer::{FirstLayerCommand};
use super::{Chunk, MessageBody};
use super::line::{LineId, LineModel};
use super::{FirstLayerHeader, ChunkType, ChannelToken};
use crate::protocol_stack::layer_pipe_item::{*};

#[derive(Clone)]
pub enum TransmissionType {
	Data,
	PingReq,
	PingAck,
}
#[derive(Clone)]
pub struct PayloadTimestamped {
	pub data: Chunk,
	pub transmit_time: Instant,
	pub chunk_type: TransmissionType,
}

#[derive(Clone, Display)]
pub enum PhysicalLayerEvent {
	Received(PayloadTimestamped),
	TimerFinished(TimerOwnerToken),
	LineOk(Chunk, Duration),
	PingLineTimeout,
	None,
}
impl PhysicalLayerEvent {
	pub fn wrap(self) -> LayerEvent {
		LayerEvent::PhysicalLayer(self)
	}
	pub fn take(&mut self) -> Self {
		std::mem::replace(self, Self::None)
	}
}

#[derive(Clone, Display)]
pub enum PhysicalLayerCommand {
	Transmit(Chunk, TransmissionType),
	StartTimer(TimerOwnerToken),
	PingLine(usize, ChannelToken),
	None,
}

impl PhysicalLayerCommand {
	pub fn wrap(self) -> LayerCommand {
		LayerCommand::PhysicalLayer(self)
	}
	pub fn take(&mut self) -> Self {
		std::mem::replace(self, Self::None)		
	}
}

impl From<PhysicalLayerCommand> for LayerPipeItem {
	fn from(cmd: PhysicalLayerCommand) -> Self {
		Self::SomeCommand(LineId::default(), LayerCommand::PhysicalLayer(cmd))
	}
}
impl From<PhysicalLayerEvent> for LayerPipeItem {
	fn from(ev: PhysicalLayerEvent) -> Self {
//		Self::SomeEvent(ev)
		Self::SomeEvent(LineId::default(), LayerEvent::PhysicalLayer(ev))
	}
}

impl From<LayerPipeItem> for PhysicalLayerCommand {
	fn from(item: LayerPipeItem) -> Self {
		match item {
			LayerPipeItem::SomeCommand(_, LayerCommand::PhysicalLayer(cmd)) => cmd,
			_ => Self::None,
		}
	}
}

impl From<LayerPipeItem> for PhysicalLayerEvent {
	fn from(item: LayerPipeItem) -> Self {
		match item {
			LayerPipeItem::SomeEvent(_, LayerEvent::PhysicalLayer(ev)) => ev,
			_ => Self::None,
		}
	}
}

pub struct LinePing {
	line_model: LineModel,
	timer_token: Option<TimerOwnerToken>,
}

impl LinePing {
    pub fn new(line_model: LineModel) -> Self {
        Self {
				line_model, timer_token: None,	
			}
    }

	fn is_ping_req(chunk: &PayloadTimestamped) -> bool {
		match chunk.chunk_type {
			TransmissionType::PingReq => true,
			_ => false,
		}
	}
/*	fn is_ping_ack(chunk: &PayloadTimestamped) -> bool {
		match chunk.chunk_type {
			TransmissionType::PingAck => true,
			_ => false,
		}
	}*/
}

impl EventTarget<LayerPipeItem> for LinePing {
	fn notify(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {

		match shared_alloc.current() {
			LayerPipeItem::SomeCommand(_, LayerCommand::PhysicalLayer(cmd)) => match cmd {
				
				PhysicalLayerCommand::PingLine(batch, channel_token) => {

					if self.timer_token.is_none() {
						let batch = *batch;
						let channel_token = *channel_token;

						let expected_delay = 2*self.line_model.probable_delay();
//							+ 2 * (batch as u32) * self.line_model.transmission_latency;
						self.timer_token = Some(TimerOwnerToken::new(expected_delay));
						
						shared_alloc.replace_propagate_current(
							(PhysicalLayerCommand::StartTimer(
								self.timer_token.as_ref().unwrap().clone())).into());
						
						shared_alloc.extend(
							(0..batch)
							.map(|_| {
								let header = FirstLayerHeader::new(
									ChunkType::Data,
									Some(channel_token));
								
								let mut chunk = Chunk::from_message(
									MessageBody::Data("line stats".to_string()));
								chunk.swaddle(header);
								PhysicalLayerCommand::Transmit(chunk, 
									TransmissionType::PingReq).into()
								})
						);
						shared_alloc.replace_propagate_current((PhysicalLayerCommand::StartTimer(
							self.timer_token.as_ref().unwrap().clone())).into());

					}
				},
				_ => (), //shared_alloc.push(cmd.into()),
			}
			LayerPipeItem::SomeEvent(_, LayerEvent::PhysicalLayer(ev)) => 
				match &ev {
					PhysicalLayerEvent::Received(chunk) => {
						let chunk = chunk.clone();

						if !self.timer_token.is_none() {
							shared_alloc.replace_propagate_current( 
								PhysicalLayerEvent::LineOk(
									chunk.data.clone(),
									chunk.transmit_time.elapsed()).into());	
						} 
						if Self::is_ping_req(&chunk) {
							shared_alloc.replace_propagate_current( 
								PhysicalLayerCommand::Transmit(
									chunk.data, 
									TransmissionType::PingAck)
								.into());				
						} 
					},
					PhysicalLayerEvent::TimerFinished(token) => {
						if !self.timer_token.is_none() &&
							*token == *self.timer_token.as_ref().unwrap() {
							self.timer_token.take();
							shared_alloc.replace_propagate_current(PhysicalLayerEvent::PingLineTimeout.into())	
						} 
					},
					_ => (), //shared_alloc.push(ev.into()),
				},
				
			_ => (),
		}
	}
}

impl From<FirstLayerCommand> for PhysicalLayerCommand {
	fn from(cmd: FirstLayerCommand) -> Self {
		match cmd {
			FirstLayerCommand::Send(mut chunk, header) => {
				log::debug!("Transmit");
				chunk.swaddle(header);

				Self::Transmit( chunk, TransmissionType::Data )
			},
			FirstLayerCommand::StartTimer(token) => Self::StartTimer(token),
			FirstLayerCommand::PingLineForStats(batch, channel_token) 
				=> Self::PingLine(batch, channel_token),
			
			_ => Self::None,
		}
	}
}
