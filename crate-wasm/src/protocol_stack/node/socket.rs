use std::cell::RefCell;
use strum_macros::{Display};
use super::{ *};
use super::super::second_layer::{*};
use super::super::physical_layer::{*};
use super::super::protocol_layers::{ProtocolStack};
use super::super::line::*;


struct SendCommandHandle {
	profile: SocketProfile,
	notify_transmit_tx:  UnboundedSender<ScheduledTransmit>,
	traffic_counter: (u32, u32),

	#[cfg(target_arch = "wasm32")]
	prev_transmit_time: instant::Instant,
	#[cfg(target_arch = "wasm32")]
	sub_millis_fraction_sim: u64,
}

pub struct Socket {
//	profile: SocketProfile,
	_node_addr: NodeAddress,
//	is_connected: bool,

	protocol_stack: RefCell<ProtocolStack>,
	send_cmd_handle: RefCell<SendCommandHandle>,

	cmds: Vec<PhysicalLayerCommand>,
	events: Vec<SecondLayerEvent>,	
}

impl Socket {
	pub fn new(
		_node_addr: NodeAddress,
		profile: SocketProfile,
		notify_transmit_tx:  UnboundedSender<ScheduledTransmit>,
	) -> Self {

		let protocol_stack = ProtocolStack::new(
			_node_addr, 
//			*profile.id.as_val_ref(),
			profile.line_model.clone(), 
			profile.max_first_layer_channels,
		);
		let send_cmd_handle = SendCommandHandle {
			profile: profile.clone(),
			notify_transmit_tx,
			traffic_counter: (0, 0),
		
			#[cfg(target_arch = "wasm32")]
			prev_transmit_time: instant::Instant::now(),
			#[cfg(target_arch = "wasm32")]
			sub_millis_fraction_sim: 0,
		};

		Self {
//			profile, 
			_node_addr,
			protocol_stack: RefCell::new(protocol_stack),
//			is_connected: false,
			send_cmd_handle: RefCell::new(send_cmd_handle),

			cmds: Vec::with_capacity(1000),
			events: Vec::with_capacity(1000),
		}
	}

/*	pub fn id(&self) -> LineTransceiverId {
		self.profile.id
	}
	pub fn line_id(&self) -> LineId {
		*self.profile.id.as_val_ref()
	}
*/	
	//pub fn line_model(&self) -> &LineModel {
	//	&self.profile.line_model
	//}

/*	pub fn is_connected(&self) -> bool {
		self.is_connected
	}
*/
	pub fn connect(&mut self) {
//		self.is_connected = true;
	}

	pub fn disconnect(&mut self) {
//		self.is_connected = false;
		self.events.clear();
		self.cmds.clear();
		
	}

/*	pub fn sample_traffic(&mut self) -> (u32, u32) {
		let sample = self.send_cmd_handle.borrow().traffic_counter;
		self.send_cmd_handle.borrow_mut().traffic_counter = (0, 0);
		sample
	}
*/
	pub fn on_command(&mut self, cmd: SecondLayerCommand) -> &mut Vec<SecondLayerEvent> {
		match cmd {
			 _ => {
				let mut stack_borrowed = self.protocol_stack.borrow_mut();

				stack_borrowed.propagate_command(cmd, &mut self.cmds, &mut self.events);

				self.send_cmd_handle.borrow_mut().send_commands_on_stream(
					&mut self.cmds
				);
			}	
		}
		&mut self.events
	}
		
	pub fn on_event(&mut self, event: PhysicalLayerEvent)
		-> &mut Vec<SecondLayerEvent> {

/*		if !self.is_connected() {
			self.events.clear();
			return &mut self.events;
			//		panic!("Socket {} is disconnected", self.id());
		}	
*/
		match event {
			PhysicalLayerEvent::Received(_) 
				=> {			
					self.send_cmd_handle.borrow_mut().traffic_counter.1 += 1;
				}
			_ => (),
		}
//		let (mut cmds, mut events) = 
		let mut stack_borrowed = self.protocol_stack.borrow_mut();
		stack_borrowed.propagate_event(event, &mut self.cmds, &mut self.events);

		self.send_cmd_handle.borrow_mut().send_commands_on_stream(&mut self.cmds);
		
		&mut self.events
	}	
}

impl SendCommandHandle {
	fn send_commands_on_stream(&mut self, cmds: &mut Vec<PhysicalLayerCommand>) {
//		fn send_commands_on_stream(&mut self, cmds: Vec<PhysicalLayerCommand>) {
/*			if !self.is_connected() {
			return;
//			panic!("Socket {} is disconnected", self.id());
		}	
*/
		while let Some(cmd) = cmds.pop() {
			log::debug!("in socket {}", cmd);
			match cmd {

				PhysicalLayerCommand::Transmit(chunk, chunk_type) => {
					self.transmit(chunk, chunk_type);
				},
				_ => ()							
			}
		}
	}
	
	#[cfg(target_arch = "wasm32")]
	fn shift_transmit_sub_millis_fraction(&mut self,
		transmit_time: &instant::Instant, 
		receive_time: &instant::Instant) -> Option<instant::Instant> {

		if self.prev_transmit_time == *transmit_time {
			self.sub_millis_fraction_sim += 1;
			receive_time.checked_add(Duration::from_nanos(self.sub_millis_fraction_sim))
		} else {
			self.sub_millis_fraction_sim = 0;
			self.prev_transmit_time = *transmit_time;
			Some(*receive_time)
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	fn shift_transmit_sub_millis_fraction(&mut self,
		transmit_time: &instant::Instant, 
		receive_time: &instant::Instant) -> Option<instant::Instant> {
			Some(*receive_time)
	}

	fn transmit(&mut self, 
		chunk: Chunk, 
		chunk_type: TransmissionType,
	) {
		let transmit_time = instant::Instant::now();
		let what = PayloadTimestamped { data: chunk, transmit_time, chunk_type };

		if let Some(receive_time) = self.profile.line_model.calc_receive_time(transmit_time) {
			if let Some(when) =
				self.shift_transmit_sub_millis_fraction(&transmit_time, &receive_time) {
		
				match self.notify_transmit_tx.unbounded_send( ScheduledTransmit {
					id: self.profile.id,
					when,
					what,
				}) {
					Ok(_) => self.traffic_counter.0 += 1,
					Err(_err) => () //panic!("{}", err),
				}
			}
//			}
		}
	}		
}


#[derive(Display)]
pub enum SocketError {
//	NotFound(LineId),
//	Disconnected(LineId),
	CommandStreamFull(LineId, SecondLayerCommand),
	CommandStreamDisconnected(LineId, SecondLayerCommand),
	NodeDisconnected(NodeAddress)
}
#[derive(Clone)]
pub struct SocketProfile {
    pub id: LineTransceiverId,
    line_model: LineModel,
    max_first_layer_channels: usize,
}

impl SocketProfile {
	pub fn new(id: LineTransceiverId,
		line_model: LineModel,
		max_first_layer_channels: usize ) -> Self {
			Self {id, line_model, max_first_layer_channels}
	}
	 
}
