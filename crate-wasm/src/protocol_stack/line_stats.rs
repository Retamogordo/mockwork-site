
use strum_macros::{Display};
//use colored::*;

use crate::pipe::{EventTarget, SharedPipeAlloc};
use super::first_layer::{FirstLayerCommand, FirstLayerEvent, LineStatistics, *};
use super::line::{LineModel};
use super::{FirstLayerHeader, ChannelToken, *};
use crate::fsm::{FSM, TransitionHandle, SignalInput, SignalIdentifier};
use super::{TimerOwnerToken};
use crate::protocol_stack::layer_pipe_item::{*};

type SignalInputItem = <LineStatsSignals as SignalInput>::Item;

struct LineStatsData {
	channel_token: Option<ChannelToken>,
	service_token: Option<ServiceToken>,
	timer_token: Option<TimerOwnerToken>,
	line_model: Option<LineModel>,
	batch: usize,
	line_statistics: Option<LineStatistics>,
	line_quality_2: f32,
	modify_pipe_items: Option<fn( &mut Self, &SignalInputItem, &mut SharedPipeAlloc<LayerPipeItem> )>,
//	modify_pipe_items: Option<fn( &mut Self, &mut SharedPipeAlloc<LayerPipeItem> )>,
}

impl LineStatsData {
    fn modify_pipe_items(
        &mut self, 
        input: &SignalInputItem,
        alloc: &mut SharedPipeAlloc<LayerPipeItem> ) {
            if let Some(closure) = self.modify_pipe_items.take() {
                (closure)(self, input, alloc);
            }
    }
}

impl Default for LineStatsData {
	fn default() -> Self {
		Self {
			channel_token: None,
			service_token: None,
			timer_token: None,
			line_model: None,
			batch: 0,
			line_statistics: None,
			line_quality_2: 0.0,
			modify_pipe_items: None,
		}
	}
}
//struct OnLineStatsChannelRequest<'a, 'b>(&'a std::marker::PhantomData<()>, &'b std::marker::PhantomData<()>); 
struct OnLineStatsChannelRequest;
impl TransitionHandle for OnLineStatsChannelRequest {
	type DataType = LineStatsData;
	type SignalType = LineStatsSignals;

	fn call(&self, input: &(), data: &mut Self::DataType) -> bool {
			//		let data = data.unwrap();

		data.channel_token = Some(ChannelToken::durable(
			3*data.line_model.as_ref().unwrap().probable_delay(),
			ChannelType::Service));
/*		
		data.pipe_items.unwrap().clear();
		data.pipe_items.unwrap().push(
			(FirstLayerCommand::ChannelRequest(
				data.channel_token.unwrap(),
				true )).into());

		data.pipe_items.push(
			FirstLayerEvent::LineStatsStarted(data.service_token.unwrap()).into());
*/
		data.modify_pipe_items = Some( |data, input, alloc| {
			alloc.take_current();

			alloc.push_done(
				FirstLayerCommand::ChannelRequest(data.channel_token.unwrap(),
				true ).wrap().into());

			alloc.push_done(	
				FirstLayerEvent::LineStatsStarted(data.service_token.unwrap()).wrap().into(),
			);
		});
		true
/*
		data.pipe_items = Some(
			vec![
				(FirstLayerCommand::ChannelRequest(
					data.channel_token.unwrap(),
					true )).wrap().into(),
				FirstLayerEvent::LineStatsStarted(data.service_token.unwrap()).wrap().into(),
				]);*/
	}
	
}


struct OnLineStatsChannelFailure; 
impl TransitionHandle for OnLineStatsChannelFailure {
	type DataType = LineStatsData;
	type SignalType = LineStatsSignals;

	fn call(&self, input: &(), data: &mut Self::DataType) -> bool {
//		let data = data.unwrap();
		data.modify_pipe_items = Some( |data, input, alloc| {
			alloc.replace_propagate_current(
				(FirstLayerEvent::LineStatsFailure(data.channel_token.take().unwrap())).wrap().into()
			);
		});
		true
/*		data.pipe_items = Some(
			vec![
				(FirstLayerEvent::LineStatsFailure(data.channel_token.take().unwrap())).wrap().into(),
				]);
				*/
	}
}

struct OnChannelEstablished; 
impl TransitionHandle for OnChannelEstablished {
	type DataType = LineStatsData;
	type SignalType = LineStatsSignals;

	fn call(&self, input: &(), data: &mut Self::DataType) -> bool {
//		let data = data.unwrap();
		// wait for channel to prime
		data.timer_token = Some(TimerOwnerToken::new(data.line_model.as_ref().unwrap().probable_delay()));	
		data.modify_pipe_items = Some( |data, input, alloc| {
			alloc.take_current();
			alloc.push_done(
				(FirstLayerCommand::StartTimer(data.timer_token.as_ref().unwrap().clone())).wrap().into(),
			);
		});
		true
/*		data.pipe_items = Some(
			vec![
				(FirstLayerCommand::StartTimer(data.timer_token.as_ref().unwrap().clone())).wrap().into(),
				]);*/
	}
}

struct OnSendingBatch; 
impl TransitionHandle for OnSendingBatch {
	type DataType = LineStatsData;
	type SignalType = LineStatsSignals;

	fn call(&self, input: &(), data: &mut Self::DataType) -> bool {
//		let data = data.unwrap();

//		println!("\tcmd: --> {}", request);
		let expected_delay = 2*data.line_model.as_ref().unwrap().probable_delay();
//					+ 2 * (data.batch as u32) * data.line_model.transmission_latency;
		data.timer_token = Some(TimerOwnerToken::new(expected_delay));

		data.line_statistics = Some(LineStatistics::new());

		data.modify_pipe_items = Some( |data, input, alloc| {
			alloc.take_current();
			alloc.push_done(
				(FirstLayerCommand::StartTimer(data.timer_token.as_ref().unwrap().clone())).wrap().into(),
			);
			alloc.push_done(
				(FirstLayerCommand::PingLineForStats(data.batch, data.channel_token.unwrap())).wrap().into()
			);
		});
		true
/*
		data.pipe_items = Some(
			vec![
				(FirstLayerCommand::StartTimer(data.timer_token.as_ref().unwrap().clone())).wrap().into(),
				(FirstLayerCommand::PingLineForStats(data.batch, data.channel_token.unwrap())).wrap().into()
				]); 
				*/
			}
}

struct OnLineStatsFinished; 
impl TransitionHandle for OnLineStatsFinished {
	type DataType = LineStatsData;
	type SignalType = LineStatsSignals;

	fn call(&self, input: &(), data: &mut Self::DataType) -> bool {
//		let data = data.unwrap();

		data.channel_token.take();
		let stats = data.line_statistics.as_mut().unwrap();
		
		stats.line_quality = data.line_quality_2.sqrt();
		stats.oneway_trip_delay /= stats.returned_chunks as u32;
//		data.line_statistics.as_mut().unwrap().line_quality = data.line_quality_2;
//		let service_token = data.service_token.take().unwrap();

//		log::info!("in OnLineStatsFinished, service oken : {}", service_token);

		data.modify_pipe_items = Some( |data, input, alloc| {
			alloc.replace_propagate_current(
				(FirstLayerEvent::LineStatsReady(
					data.service_token.take().unwrap(),
					data.line_statistics.take().unwrap())).wrap().into(),
			);
		});
/*
		data.pipe_items = Some(
			vec![
				(FirstLayerEvent::LineStatsReady(
					service_token,
					data.line_statistics.take().unwrap())).wrap().into(),
				]);*/
		data.line_quality_2 = 0.0;		
		true
	}
}

#[derive (PartialEq, Eq, Hash, Clone)]
enum LineStatsSignals {
	ChannelRequestSignal,
	ChannelTimeoutSignal,
	TimerFinishedSignal,
	ChannelEstablishedSignal,
//	LineStatsFinishedSignal,
}

impl SignalInput for LineStatsSignals {
	type Item = ();
	fn get(&self) -> &Self::Item {
		&()
	}
	fn set(&mut self, data: Self::Item) {
		
	}
}

impl SignalIdentifier for LineStatsSignals {
    type IdType = Self;
    fn id(&self) -> Self::IdType {
		self.clone()
    }
}

#[derive (PartialEq, Clone, Copy, Display)]
enum LineStatsStates {
	WaitingForChannelState,
	WaitingForChannelPrimedState,
	SendingBatchState,
}

use LineStatsSignals::*;
use LineStatsStates::*;

pub struct LineStats {
//	line_model: LineModel,
	fsm: FSM<LineStatsStates, LineStatsSignals, LineStatsData>,
//	state_data: LineStatsData,
}

impl LineStats {
	pub fn new(line_model: LineModel) -> Self {
		let mut fsm = 
				FSM::<LineStatsStates, LineStatsSignals, LineStatsData>::from_vec(vec![
					WaitingForChannelState,
					WaitingForChannelPrimedState,
					SendingBatchState,
				]);
		fsm.chain_awaiting_state()
			.chain(LineStatsSignals::ChannelRequestSignal, WaitingForChannelState, 
				Some(Box::new(OnLineStatsChannelRequest)))
			.chain_to_awaiting_state(ChannelTimeoutSignal,
				Some(Box::new(OnLineStatsChannelFailure)));
		
		fsm.start_chain(WaitingForChannelState)
			.chain(ChannelEstablishedSignal, WaitingForChannelPrimedState,
				Some(Box::new(OnChannelEstablished)))
			.chain(TimerFinishedSignal, SendingBatchState,
				Some(Box::new(OnSendingBatch)))
			.chain_to_awaiting_state(TimerFinishedSignal,
				Some(Box::new(OnLineStatsFinished)));
	

		let state_data = LineStatsData {
//			pipe_items: None,
			channel_token: None,
			service_token: None,
			timer_token: None,
			line_model: Some(line_model),
			batch: 0,
			line_statistics: None,
			line_quality_2: 0.0,
			modify_pipe_items: None,
		};
		fsm.init(state_data);
		fsm.run();

		Self {  fsm, 
//			state_data 
		}
	}

	fn timer_finished(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
//		state_data!(self, pipe_items).take();
//		self.state_data.foo.take();
		self.fsm
			.signal(&TimerFinishedSignal)
			.modify_pipe_items(&(), shared_alloc);
	}

	fn prime_channel(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
		self.fsm
			.signal(&ChannelEstablishedSignal)
			.modify_pipe_items(&(), shared_alloc);
	}

	fn gather_stats(&mut self, trip_delay: Duration) {
//	fn gather_stats(&mut self, chunk: Chunk, received_at: SystemTime) {
		if let Some(stats) = self.fsm.state_data_mut().line_statistics.as_mut() {
//			self.fsm.state_data.line_quality_2 += 1.0/self.state_data.batch as f32;
		
			stats.returned_chunks += 1;

			stats.oneway_trip_delay += trip_delay;
		}
	}

	fn on_channel_failure(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
		self.fsm
			.signal(&ChannelTimeoutSignal)
			.modify_pipe_items(&(), shared_alloc);
	}
}

impl EventTarget<LayerPipeItem> for LineStats {

	fn notify(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {

		match shared_alloc.current() {
		LayerPipeItem::SomeEvent(_, LayerEvent::FirstLayer(event)) => 
			match event {
				FirstLayerEvent::ChunkReceived(chunk, _at) => {
					let hdr: &FirstLayerHeader = chunk.header().into();
	
					if self.fsm.state_data().channel_token.is_none()
						|| hdr.channel_token != self.fsm.state_data().channel_token {
	
//						shared_alloc.replace_propagate_current(
//							FirstLayerEvent::ChunkReceived(*chunk, *_at).into());
					}
				},
	
				FirstLayerEvent::HandshakeFailure(channel_token) => {
					let channel_token = *channel_token;
					if Some(channel_token) == self.fsm.state_data().channel_token {
						self.on_channel_failure(shared_alloc);
					} else {
	//					state_data!(self, pipe_items) = Some(vec![
	//						FirstLayerEvent::HandshakeFailure(channel_token).into()])
	//					shared_alloc.clear();
	//					shared_alloc.push(FirstLayerEvent::HandshakeFailure(channel_token).into());
	//					shared_alloc.push(event.into());
					}
				}
				
				FirstLayerEvent::LineOk(ref chunk, trip_delay) => {
					let hdr: &FirstLayerHeader = chunk.header().into();
	
					if hdr.channel_token == self.fsm.state_data().channel_token
						&& !self.fsm.state_data().channel_token.is_none() {
							
							self.gather_stats(*trip_delay);
					} else {
	//					shared_alloc.push(event.into());
	//					state_data!(self, pipe_items) = Some(vec![
	//						FirstLayerEvent::LineOk(chunk, trip_delay).into()])
					}
				},
	
				FirstLayerEvent::ChannelEstablished(token) => {
//					let channel_token = *channel_token;
					if Some(*token) == self.fsm.state_data().channel_token  {				
						self.prime_channel(shared_alloc);						
					} 
					else {
	//					shared_alloc.push(event.into());
					}
				//	self.on_received(chunk);
				},
	
				FirstLayerEvent::TimerFinished(ref token) => {
					if Some(token.clone()) == self.fsm.state_data().timer_token {
						self.timer_finished(shared_alloc);						
					}
					else {
	//					shared_alloc.push(event.into());
					} 
					/*else {
						state_data!(self, pipe_items) = Some(vec![
							FirstLayerEvent::TimerFinished(token).into()
							]);	
					}*/
				},
	
				FirstLayerEvent::ChannelExpired(channel_token) => {
					// just drop own channel here
					let channel_token = *channel_token;
					if Some(channel_token) == self.fsm.state_data().channel_token {
	//					shared_alloc.clear();
					} else {
	//					shared_alloc.push(event.into());
					}
					/*
						if Some(channel_token) != state_data!(self, channel_token) {
						state_data!(self, pipe_items) = Some(vec![
							FirstLayerEvent::ChannelExpired(channel_token).into()
						]);	
					}*/
				}
	
				_ => (), //shared_alloc.push(event.into()), // state_data!(self, pipe_items) = Some(vec![event.into()]),
			},
	
			LayerPipeItem::SomeCommand(_, LayerCommand::FirstLayer(cmd)) => 
				match cmd {
					FirstLayerCommand::LineStats(service_token, batch) => {
/*						self.state_data.batch = *batch;
						self.state_data.service_token = Some(*service_token);*/
						self.fsm
							.signal(&LineStatsSignals::ChannelRequestSignal)
							.modify_pipe_items(&(), shared_alloc);					
					}
					
					_ => () //shared_alloc.push(cmd.into()), // state_data!(self, pipe_items) = Some(vec![cmd.into()]),
				}
				_ => unimplemented!(),
		}
	}
}
