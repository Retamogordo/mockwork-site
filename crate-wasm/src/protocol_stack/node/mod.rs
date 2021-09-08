pub mod stream;
pub mod node_services;
pub mod socket;

use futures::channel::mpsc::{unbounded, UnboundedSender, UnboundedReceiver };
use futures::channel::{oneshot};
//#[cfg(not(target_arch = "wasm32"))]
//use async_std::task:: {block_on};
use futures::StreamExt;
use std::sync::{Arc, RwLock};

use std::sync::atomic::{Ordering, AtomicU32, AtomicUsize};
use std::fmt::Display;
use std::collections::HashMap;
use strum_macros::{Display};
use core::time::{Duration};
use futures::{pin_mut, select, FutureExt};
use serde::{Serialize};

use super::{ ChannelToken, TimerOwnerToken};
use self::stream::{StreamHandle, AcquireStreamHandle, IncomingHandle, ServiceManageHandle, service_stream, ChannelError};
use self::socket::{*};
use super::second_layer::{*};
use super::physical_layer::{*};
use super::protocol_layers::{NodeProtocolStack};
use super::line::*;
//use crate::target_dependant::{TargetDependantJoinHandle};
use crate::protocol_stack::{Chunk};
use crate::mock_work::{ScheduledTransmit};
//use crate::{warn_on_error};
use crate::protocol_stack::layer_pipe_item::{*};
//#[cfg(target_arch = "wasm32")]
use crate::target_dependant::{CancellableRun, spawn_cancellable};

pub type IncomingStreamSender = UnboundedSender<StreamHandle>;
pub type IncomingStreamReceiver = UnboundedReceiver<StreamHandle>;

pub type LineEventSender = UnboundedSender<(LineId, PhysicalLayerEvent)>;
pub type LineEventReceiver = UnboundedReceiver<(LineId, PhysicalLayerEvent)>;

type NodeEventSender = UnboundedSender<NodeEvent>;
type NodeEventReceiver = UnboundedReceiver<NodeEvent>;
type NodeCommandSender = UnboundedSender<NodeCommand>;
type NodeCommandReceiver = UnboundedReceiver<NodeCommand>;

pub type TimerSender = UnboundedSender<(LineId, TimerOwnerToken)>;
pub type TimerReceiver = UnboundedReceiver<(LineId, TimerOwnerToken)>;

#[derive(Display)]
enum NodeChannelError {
//	CommandStreamFull(LineTransceiverId, PhysicalLayerCommand),
//	CommandStreamDisconnected(LineTransceiverId, PhysicalLayerCommand),
	EventStreamFull(NodeEvent),
	EventStreamDisconnected(NodeEvent),
}

#[derive(Display)]
pub(crate) enum NodeStatus {
	Connected = 1,
	Connecting,
	Suspending,
	Suspended,
	Disconnecting,
	Disconnected,
}

impl From<usize> for NodeStatus {
    fn from(n: usize) -> Self {
        match n {
            1 => Self::Connected,
			2 => Self::Connecting,
            3 => Self::Suspending,
            4 => Self::Suspended,
            5 => Self::Disconnecting,
            6 => Self::Disconnected,
            _ => panic!("Unknown Mock Work status"),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct NodeAddress(u32, u32);

impl NodeAddress {
		pub const ANY: NodeAddress = NodeAddress(0u32, 0u32);

        pub fn new(hi: u32, lo: u32) -> Self {
        Self (hi, lo)
    } 
}

impl Default for NodeAddress {
	fn default() -> Self {
		Self::ANY
	}
}

impl Display for NodeAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "|{}.{}|", self.0, self.1)
    }
}

impl From<String> for NodeAddress {
	fn from(s: String) -> Self {
		let mut iter = s.split(|c| c == '|' || c == '.');
		iter.next();
		Self (
			iter.next().unwrap().parse::<u32>().unwrap(),
			iter.next().unwrap().parse::<u32>().unwrap()
		)
	}
}

impl Serialize for NodeAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer, {
	let state = serializer.serialize_str(&self.to_string())?;
		Ok(state)
    }
}

#[derive(Clone, Copy)]
pub struct AddressMapEntry {
    pub line_id: LineId,
    pub oneway_delay: Duration
}

impl std::fmt::Display for AddressMapEntry {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "-- {} ({} ms) -->", self.line_id, self.oneway_delay.as_millis())
	}
}

#[derive(Display)]
enum ControlCommand {
//    Suspend,
	ConnectLine(LineId),
	DisconnectLine(LineId),
//	StopExpiredStreamsCleaner,
//    Shutdown,
}


pub struct Node {
    addr: NodeAddress,
	status: Arc<AtomicUsize>,
	socket_profiles: Arc<Vec<SocketProfile>>,
	node_frontend_event_loop_cancel_handle: Option<CancellableRun>,
	notify_transmit_tx: UnboundedSender<ScheduledTransmit>,
	service_manager_tx: Option<UnboundedSender<ServiceManageHandle>>,
	node_cmd_tx: Option<NodeCommandSender>,
	line_event_tx: Option<LineEventSender>,
	control_cmd_tx: Option<UnboundedSender<ControlCommand>>,
//	event_listener_loop_handle: Option<TargetDependantJoinHandle<()>>,
	incoming_stream_notify_rx: Option<Arc<async_std::sync::Mutex<
		IncomingStreamReceiver>>>,
	address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,
	js_loop_cycle_gap: Arc<AtomicU32>,
	agent_event_loop_join_handle: Option<oneshot::Receiver<()>>,
}

impl Node {
	pub fn new(
		addr: NodeAddress, 
		socket_profiles: Vec<SocketProfile>,
		notify_transmit_tx: UnboundedSender<ScheduledTransmit>,
        js_loop_cycle_gap: Arc<AtomicU32>,
	) -> Self {

		let address_map = Arc::new(RwLock::new(HashMap::new()));

		let socket_profiles = Arc::new(socket_profiles);

		Self { 
			addr,
			status: Arc::new(AtomicUsize::new(NodeStatus::Disconnected as usize)),
			socket_profiles: Arc::clone(&socket_profiles),
			node_frontend_event_loop_cancel_handle: None,
			notify_transmit_tx,
			service_manager_tx: None,
			node_cmd_tx: None,
			line_event_tx: None,
			control_cmd_tx: None,
//			event_listener_loop_handle: None,
			incoming_stream_notify_rx: None,
			address_map,
			js_loop_cycle_gap,
			agent_event_loop_join_handle: None,
		}
	}

	pub fn addr(&self) -> NodeAddress {
		self.addr
	}

	pub fn socket_profiles(&self) -> &Vec<SocketProfile>  {
		&*self.socket_profiles
	}

	pub fn line_event_sender(&self) -> Option<&LineEventSender> {
		self.line_event_tx.as_ref()
	}

	pub async fn resume(&mut self)  {
		match self.status.load(Ordering::Relaxed).into() {
			NodeStatus::Suspended => {
				self.status.store(NodeStatus::Connecting as usize, Ordering::Relaxed);
				self.connect_inner().await;
			},
			_ => () 
		}
	}
		
	pub async fn suspend(&mut self) {
		match self.status.load(Ordering::Relaxed).into() {
			NodeStatus::Connected => {
				self.status.store(NodeStatus::Suspending as usize, Ordering::Relaxed);
				self.line_event_tx.take();
				self.node_frontend_event_loop_cancel_handle.take();
				let _ = self.agent_event_loop_join_handle.take().unwrap().await;
			},
			_ => ()
		}
	}

	pub async fn connect(&mut self) -> bool {
		match self.status.load(Ordering::Relaxed).into() {
			NodeStatus::Disconnected => {
				self.status.store(NodeStatus::Connecting as usize, Ordering::Relaxed);
				self.connect_inner().await;
				log::debug!("after connect inner");
				true
			},
			_ => false,
		}
	}

	pub async fn disconnect(&mut self) {
		self.status.store(NodeStatus::Disconnecting as usize, Ordering::Relaxed);
		self.line_event_tx.take();
		self.node_frontend_event_loop_cancel_handle.take();
		let _ = self.agent_event_loop_join_handle.take().unwrap().await;
	}

	async fn connect_inner(&mut self) {
		let lines: Arc<Vec<LineId>> = Arc::new(self.socket_profiles.iter()
			.map(|sp| {
				*sp.id.clone().as_val_ref()
			})
			.collect()
		);

		let (line_event_tx, line_event_rx): (LineEventSender, LineEventReceiver) = unbounded();
		let (node_cmd_tx, node_cmd_rx): (NodeCommandSender, NodeCommandReceiver) = unbounded();
		let (node_event_tx, node_event_rx): (NodeEventSender, NodeEventReceiver) = unbounded();		
		let (service_manager_tx, service_manager_rx): 
			(UnboundedSender<ServiceManageHandle>, UnboundedReceiver<ServiceManageHandle>) = unbounded();

		let node_handle = NodeStateHandle { 
			lines: Arc::clone(&lines),
			address_map: Arc::clone(&self.address_map),
			events: Vec::with_capacity(1000),
			cmds: Vec::with_capacity(1000),
		};

		let (incoming_stream_notify_tx, incoming_stream_notify_rx):
			(IncomingStreamSender, IncomingStreamReceiver) = unbounded();
		let (control_cmd_tx, control_cmd_rx):
			(UnboundedSender<ControlCommand>, UnboundedReceiver<ControlCommand>) = unbounded();

		self.line_event_tx = Some(line_event_tx);
		self.node_cmd_tx = Some(node_cmd_tx.clone());
		self.service_manager_tx = Some(service_manager_tx);
		self.incoming_stream_notify_rx = Some(
				Arc::new(async_std::sync::Mutex::new(incoming_stream_notify_rx)));
		self.control_cmd_tx = Some(control_cmd_tx.clone());
		
		let status = Arc::clone(&self.status);
		let addr = self.addr;	
		let (agent_event_loop_join_handle_tx, agent_event_loop_join_handle_rx):
			(oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel(); 
		self.agent_event_loop_join_handle = Some(agent_event_loop_join_handle_rx);
		
		let (agent_event_loop_started_tx, agent_event_loop_started_rx):
			(oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel(); 
		
		let mut agent_event_loop_cancel_handle = Some(
			spawn_cancellable(
				agent_event_loop(
					self.addr,
					Arc::clone(&self.socket_profiles),
					line_event_rx,
					node_cmd_rx,
					control_cmd_rx,
					node_event_tx,
					self.notify_transmit_tx.clone(),
					node_handle,
					Arc::clone(&self.js_loop_cycle_gap),
					agent_event_loop_started_tx,
				),
				move || {
					match status.load(Ordering::Relaxed).into() {
						NodeStatus::Disconnecting 
							=> status.store(NodeStatus::Disconnected as usize, Ordering::Relaxed),
						NodeStatus::Suspending 
							=> status.store(NodeStatus::Suspended as usize, Ordering::Relaxed),
						_ => (),
					}
					log::debug!(">>>>>>>>> {} agent_event_loop over, node status: {}", addr, 
						NodeStatus::from(status.load(Ordering::Relaxed)));
					let _ = agent_event_loop_join_handle_tx.send(());
				} )
		);

		let (node_frontend_event_loop_started_tx, node_frontend_event_loop_started_rx):
			(oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel(); 

		self.node_frontend_event_loop_cancel_handle 
			= Some(spawn_cancellable(
				node_frontend_event_loop(
					self.addr,
					node_cmd_tx,
					node_event_rx,
					service_manager_rx,
					incoming_stream_notify_tx,
					Arc::clone(&self.js_loop_cycle_gap),
					node_frontend_event_loop_started_tx,
				),
				move || { 
					agent_event_loop_cancel_handle.take();
					log::debug!("-------node_frontend_event_loop over") 
				})
			);

		if let ( Ok(_), Ok(_) ) = futures::future::join( 
			agent_event_loop_started_rx, 
			node_frontend_event_loop_started_rx )
		.await {

			for sp in self.socket_profiles.iter() {
				if let Some(ref control_cmd_tx) = self.control_cmd_tx {
				let _res = 
					control_cmd_tx.unbounded_send(
						ControlCommand::ConnectLine(*sp.id.as_val_ref()));
				}
			}
			self.status.store(NodeStatus::Connected as usize, Ordering::Relaxed);
		}
	}
	#[allow(dead_code)]	
	fn toggle_line_connection(&mut self, line_id: &LineId, connect: bool) {
		match self.status.load(Ordering::Relaxed).into() {
			NodeStatus::Connected => {
				if let Some(_) = self.socket_profiles().iter()
					.find(|&sp| *sp.id.as_val_ref() == *line_id) {
						log::debug!("node {} connecting line {}", self.addr, line_id);
						let _res = 
							self.control_cmd_tx.as_mut().unwrap().unbounded_send(
								if connect { ControlCommand::ConnectLine(*line_id) }
								else { ControlCommand::DisconnectLine(*line_id) }
							);
				}
			},
			_ => ()
		}
	}

	pub fn is_connected(&self) -> bool {
		match self.status.load(Ordering::Relaxed).into() {
			NodeStatus::Connected
			|
			NodeStatus::Suspending
			|
			NodeStatus::Suspended => true,
			_ => false,
		}
	}

	pub(crate) fn status(&self) -> NodeStatus {
		NodeStatus::from(self.status.load(Ordering::Relaxed))
	}

	pub fn command(&mut self, cmd: NodeCommand) -> Result<(), SocketError> {

		if let Some(ref node_cmd_tx) = self.node_cmd_tx {
			node_cmd_tx.unbounded_send(cmd)
				.map_err(|err| 
					if err.is_full() {
						let cmd = err.into_inner();
						SocketError::CommandStreamFull(cmd.0, cmd.1)
					} else {
						let cmd = err.into_inner();
						SocketError::CommandStreamDisconnected(cmd.0, cmd.1)
					})
		} else {
			Err(SocketError::NodeDisconnected(self.addr)) 
		}
	}

	pub fn nodes_online(&self) -> HashMap<NodeAddress, LineId> {
		self.address_map.read().unwrap().iter()
			.map(|(&addr, entry)| (addr, entry.line_id)).collect()
	}

	pub fn neighbours_online(&self, line_id: LineId) -> Vec<(NodeAddress, LineId)> {
		self.address_map.read().unwrap().iter()
			.filter(|(_, entry)| entry.line_id == line_id)
			.map(|(&addr, entry)| (addr, entry.line_id))
			.collect()
	}

	pub fn path_delay(&self, dest: &NodeAddress) -> Option<Duration> {
		self.address_map.read().unwrap().get(dest)
			.and_then(|entry| Some(entry.oneway_delay))
	}

	pub fn incoming(&self) -> IncomingHandle {
		let clone = Arc::clone(&self.incoming_stream_notify_rx.as_ref().unwrap());
		IncomingHandle::new( clone )
	}


	pub async fn shutdown(&mut self) {
		self.disconnect().await;
	}
}

impl Drop for Node {
	fn drop(&mut self) {
//		self.shutdown();
		log::debug!("Node {} dropped", self.addr);
	}
}

impl Display for Node {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		//   	write!(f, "{}", self.stringify())
		write!(f, "{}", self.addr)
   }	   
}

impl std::fmt::Debug for Node {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		//   	write!(f, "{}", self.stringify())
		write!(f, "{}, {}", self.addr, 
			NodeStatus::from(self.status.load(Ordering::Relaxed)))
   }	   
}

macro_rules! send_them_on_streams {
	($node_state_handle: ident, 
		$protocol_stack: ident,
		$sockets: ident,
//		$cmds_iter: ident,
//		$events_iter: ident,
		$node_event_tx: ident,
		$timer_tx: ident) => {

		while let Some(item) = $node_state_handle.cmds.pop() {
//		for item in $cmds_iter {
			match item {
				LayerPipeItem::SomeCommand(line_id, 
					LayerCommand::SecondLayer(SecondLayerCommand::StartTimer(token)) 
				) => {
					crate::target_dependant::spawn(start_timer($timer_tx.clone(), line_id, token));
					continue;
				},

				LayerPipeItem::SomeCommand(line_id, LayerCommand::SecondLayer(cmd)) =>
				{
					if let Some(socket) = $sockets.get_mut(&line_id) {				
	//					let mut locked = socket.lock().unwrap();
						let socket_events = socket.on_command(cmd);
//						let socket_events = socket.recent_events();
		
						while let Some(ev) = socket_events.pop() {
							$protocol_stack.propagate_event(NodeEvent(line_id, ev),
								&mut $node_state_handle.cmds,
								&mut $node_state_handle.events
							);
						}				
					}		
				}
				_ => (),
			};
//			log::info!("before on command {}, line: {}, sockets len: {}", cmd.1, cmd.0,
//				$node_state_handle.sockets.len() );						
		}
		while let Some(item) = $node_state_handle.events.pop() {
			match item {
				LayerPipeItem::SomeEvent(lid, event) => {
//					warn_on_error!(
					let _ = $node_event_tx.unbounded_send(NodeEvent(lid, event.into()))
						.map_err(|err| 
							if err.is_full() {
								NodeChannelError::EventStreamFull(err.into_inner())
							} else {
								NodeChannelError::EventStreamDisconnected(err.into_inner())
							}
					);},
				//),
				_ => (),
			}
		}
	};
}

async fn start_timer(timer_tx: TimerSender, line_id: LineId, token: TimerOwnerToken) {
	crate::target_dependant::delay(token.delay()).await;

//	warn_on_error!(
	let _ = timer_tx.unbounded_send((line_id, token));
//	); 
}

async fn agent_event_loop(
	node_addr: NodeAddress,
	socket_profiles: Arc<Vec<SocketProfile>>,
	mut line_event_rx: LineEventReceiver,
	mut node_cmd_rx: NodeCommandReceiver,
	mut control_cmd_rx: UnboundedReceiver<ControlCommand>,
	node_event_tx: NodeEventSender,
	notify_transmit_tx: UnboundedSender<ScheduledTransmit>,
	mut node_state_handle: NodeStateHandle,
	js_loop_cycle_gap: Arc<AtomicU32>,
	started_tx: oneshot::Sender<()>,
)  {

	let mut sockets = 
		socket_profiles
			.iter()
			.map(|socket_profile| (*socket_profile.id.as_val_ref(), 
				Socket::new(node_addr, socket_profile.clone(), notify_transmit_tx.clone())))
			.collect::<HashMap<LineId, Socket>>();
	
	let mut protocol_stack = NodeProtocolStack::new(
		node_addr,
		Arc::clone(&node_state_handle.lines),
		Arc::clone(&node_state_handle.address_map),
	);


	let (timer_tx, mut timer_rx): (TimerSender, TimerReceiver) = unbounded();
	
	let mut prev_js_tick = instant::Instant::now();
	let mut _cmds_recv = 0;
	let mut _evs_recv = 0;

	if started_tx.send(()).is_err() { return; }

	loop {
		let js_loop_cycle_gap = js_loop_cycle_gap.load(Ordering::Relaxed);

		let cmd_task = node_cmd_rx.next().fuse();
		let event_task = line_event_rx.next().fuse();		
		let timer_task = timer_rx.next().fuse();
		let control_cmd_task = control_cmd_rx.next().fuse();
		
		pin_mut!(cmd_task, event_task, timer_task, control_cmd_task);

		select! {
			ctrl = control_cmd_task => {
				if let Some(ctrl) = ctrl {
					match ctrl {

						ControlCommand::ConnectLine(line_id) => {
							sockets
								.get_mut(&line_id)
								.and_then(|socket| {
									let _res = node_event_tx.unbounded_send(
										NodeEvent(
											line_id, 
											SecondLayerEvent::OnSocketConnected));
		
									Some(socket.connect())
								});
						},
						ControlCommand::DisconnectLine(line_id) => {
							sockets
								.get_mut(&line_id)
								.and_then(|socket| Some(socket.disconnect()));

							let _res = node_event_tx.unbounded_send(
								NodeEvent(
									line_id, 
									SecondLayerEvent::OnSocketShutdown));
						},
					}
				} else { break; }
			}

			cmd = cmd_task => {
				if let Some(cmd) = cmd {
					_cmds_recv += 1;
//					let line_id = cmd.0;
					match cmd.1  {

						_ => {
							protocol_stack.propagate_command(cmd, 
								&mut node_state_handle.cmds,
								&mut node_state_handle.events
							);

							send_them_on_streams!(
								node_state_handle, 
								protocol_stack,
								sockets,
								node_event_tx, 
								timer_tx);
						},
					}
				} else { break; } 
			},
			event = event_task => { 
				if let Some(event) = event {
					_evs_recv += 1;
					let ev = event.1; // {
					let line_id = event.0;

					if let Some(socket) = sockets.get_mut(&line_id) {

						let socket_events = socket.on_event(ev);
						while let Some(ev) = socket_events.pop() {
							protocol_stack.propagate_event( 
									NodeEvent(line_id, ev),
									&mut node_state_handle.cmds,
									&mut node_state_handle.events
							);
						}
						send_them_on_streams!(
							node_state_handle, 
							protocol_stack,
							sockets,
							node_event_tx, 
							timer_tx);
					}
				} else { break; }
			},

			token = timer_task => {
				if let Some((line_id, token)) = token {
//					log::debug!("In Node agent loop - Timer Event");
					if let Some(socket) = sockets.get_mut(&line_id) {
						
						let socket_events = socket.on_event(PhysicalLayerEvent::TimerFinished(token));
						while let Some(ev) = socket_events.pop() { 
							protocol_stack.propagate_event(
								NodeEvent(line_id, ev), 
								&mut node_state_handle.cmds, 
								&mut node_state_handle.events
							);
						}
					} 
					else if line_id == LineId::default() {
						let ev = NodeEvent(line_id, SecondLayerEvent::TimerFinished(token));
						protocol_stack.propagate_event(
							ev, 
							&mut node_state_handle.cmds, 
							&mut node_state_handle.events
						);
					}

					send_them_on_streams!(
						node_state_handle, 
						protocol_stack,
						sockets,
						node_event_tx, 
						timer_tx);
				} else { break; }
			},
/*			default => {
			//	defs += 1;
				crate::target_dependant::run_on_next_js_tick().await;	
			}*/
		}

		if prev_js_tick.elapsed().as_millis() >= js_loop_cycle_gap as u128 {
			_cmds_recv = 0;
			_evs_recv = 0;
			crate::target_dependant::run_on_next_js_tick().await;
			prev_js_tick = instant::Instant::now();
		}

	}
//	node_state_handle.node_connected.store(false, Ordering::Relaxed);
//	log::warn!("-------- {} after agent loop", node_addr);
}

async fn node_frontend_event_loop(
	node_addr: NodeAddress,
	node_cmd_tx: NodeCommandSender,
    mut event_rx: UnboundedReceiver<NodeEvent>,
	mut service_manager_rx: UnboundedReceiver<ServiceManageHandle>,
    incoming_stream_notify_tx: IncomingStreamSender,
	js_loop_cycle_gap: Arc<AtomicU32>,
	started_tx: oneshot::Sender<()>,
)  {
//    const SERVICE_ACTIVATION_TOLERANCE_MILLIS: u128 = 10000;
    let mut stream_data_senders: HashMap<ChannelToken, ServiceManageHandle>
									= HashMap::new();

	let mut prev_js_tick = instant::Instant::now();

	if started_tx.send(()).is_err() { return; }
//	log::info!("node {} reconnected frontend", node_addr);
//	let mut expired_streams_collector_delay = None;

	loop {
		let js_loop_cycle_gap = js_loop_cycle_gap.load(Ordering::Relaxed);

		let service_manager_task = service_manager_rx.next().fuse();
		let event_task = event_rx.next().fuse();	
//		let mut expired_streams_collector_task = crate::target_dependant::delay(Duration::from_secs(1_000_000)).fuse();

/*		if expired_streams_collector_delay.is_none() {
			if stream_data_senders.len() > 0 {
				expired_streams_collector_delay = Some(Duration::from_millis(100));
				expired_streams_collector_task 
					= crate::target_dependant::delay(expired_streams_collector_delay.unwrap()).fuse();
			} 
		}*/
		let _res = 
		if stream_data_senders.len() > 0 {
			node_cmd_tx.unbounded_send(
				NodeCommand(LineId::default(), 
					SecondLayerCommand::StartExpiredStreamsCleaner(
						Duration::from_millis(50)))
			)} else {
				node_cmd_tx.unbounded_send(
					NodeCommand(LineId::default(), SecondLayerCommand::StopExpiredStreamsCleaner)
			)};

//		let timer_task = timer_rx.next().fuse();
		pin_mut!(service_manager_task, event_task);

		select! {
			service = service_manager_task => {
				if let Some(service_manage_handle) = service {

					stream_data_senders
						.entry(*service_manage_handle.token())
						.or_insert( service_manage_handle );
				} else { break; }
			},

			event = event_task => {
				if let Some(event) = event {
					let line_id = event.0;
 //					log::info!("Node {}: event: {} from line {}", node_addr, event, line_id);					

					match event.1 {
						SecondLayerEvent::OnSocketShutdown => {
							// need to drop handles before cmd sender is gone,
							// so DropService commands can be propagated to corresponding streams
							// such that they do not stay there after possible socket reconnection later
							stream_data_senders.retain(
								|_, service_manage_handle| service_manage_handle.line_id != line_id )
						},
			
						SecondLayerEvent::ServiceStarted(service_token) => {
							let stream_data_tx = stream_data_senders.get_mut(&service_token);
			
							log::debug!("Node {}: event: {} from line {}, token {}", 
								node_addr, "service started", line_id, service_token);

							if let Some(service_manage_handle) = stream_data_tx {
								match service_manage_handle.activate_stream() {
									Ok(_) => (),
									Err(err) => { 
										let mut handle 
											= stream_data_senders.remove(&service_token).unwrap();
										handle.set_drop_reason(err.1);	
									},
								}
							}
						},
			
						SecondLayerEvent::ServiceMessage(service_token, service_res) => {
//							log::info!("Node {}: event: {} from line {}, token {}", 
//								node_addr, "service msg", line_id, service_token);
							let stream_data_tx = stream_data_senders.get_mut(&service_token);
			
							if let Some(service_manage_handle) = stream_data_tx {
								match service_manage_handle.send(service_res) {
									Err(err) => {
										let mut handle 
											= stream_data_senders.remove(&service_token).unwrap();
										handle.set_drop_reason(err.1);	
									}
									_ => (),
								}
							}
						}
			
						SecondLayerEvent::ServiceComplete(service_token, service_res) => {
//							log::warn!("Node {}: event: {} from line {}, token {}", 
//								node_addr, "service complete", line_id, service_token);

							if let Some(mut service_manage_handle) 
										= stream_data_senders.remove(&service_token) {
							
								match service_res {
									ServiceResultType::LineChannel(res) => 
										service_manage_handle.set_drop_reason(res),
									
									_ => {
										let _res = service_manage_handle.send(service_res);
										service_manage_handle.set_drop_reason(LineChannelResult::Unspecified);
									}
								}								
							}
						},

						SecondLayerEvent::IncomingPeerChannel(src_addr, channel_token) => {
				//			if channel_token.channel_type == ChannelType::Data &&
							if !stream_data_senders.contains_key(&channel_token) {	
								
								log::debug!("\t\tCalling service_stream from {} IncomingPeerChannel {}", node_addr, channel_token);
								let (acq_stream_handle, mut service_manage_handle) 
									= service_stream(node_addr, 
										src_addr,
										line_id,
										channel_token, 
										node_cmd_tx.clone());
								
								if let Ok(_) = service_manage_handle.activate_stream() {
									let stream_handle = acq_stream_handle.yield_stream();
									
									stream_data_senders.insert(*service_manage_handle.token(), 
										service_manage_handle );
							
									incoming_stream_notify_tx
										.unbounded_send(stream_handle)
										.unwrap_or_else(|_| { stream_data_senders.remove(&channel_token); });
								} else {
									stream_data_senders.remove(&channel_token);
								}
							}
						}
/*
						SecondLayerEvent::TimeToCollectExpiredStreams => {
//							log::info!("CLEANER IS RUNNING ON {}", node_addr);

							stream_data_senders.retain(
								|_, service_manage_handle| { 
									if service_manage_handle.token().expired() {
//										log::info!("expired dropped {}", service_manage_handle.token());
										service_manage_handle.set_drop_reason(LineChannelResult::Expired);
										false
									}
									else 
									if !service_manage_handle.activated() &&
										service_manage_handle.registered_at().elapsed().as_millis() 
											> SERVICE_ACTIVATION_TOLERANCE_MILLIS {
										log::warn!("NEVER ACTIVATED DETECTED {}", 
											service_manage_handle.token()
										);
										service_manage_handle.set_drop_reason(
											LineChannelResult::NeverActivated);
										false
									} 
									else {true}
								});
						},
*/
						_ => (),
					}
				} 
				else {
					log::debug!("break on None event");
					break; // break on agent's shutdown
				}
			},
/*			() = expired_streams_collector_task => {
				expired_streams_collector_delay.take();
				for lid in lines.iter() {
					if node_cmd_tx.unbounded_send(
						NodeCommand(*lid, SecondLayerCommand::CheckExpiredTokens))
						.is_err() {
							panic!("");
							break;
						}
				}

			}*/
/*			default => {
				crate::target_dependant::run_on_next_js_tick().await;	
			},	*/		
		}

		if prev_js_tick.elapsed().as_millis() >= js_loop_cycle_gap as u128 {
			crate::target_dependant::run_on_next_js_tick().await;
			prev_js_tick = instant::Instant::now();
		}

    }

	log::debug!("Node {}: AFTER FRONTEND LOOP", node_addr);
}

struct NodeStateHandle {
	lines: Arc<Vec<LineId>>,
	address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,
	cmds: Vec<LayerPipeItem>,
	events: Vec<LayerPipeItem>,
}
/*
pub fn display_addr_map(node: &Node) {
	let guard = node.address_map.read().unwrap();
	log::info!("Address Map for {}", node.addr);
	for (addr, entry) in (*guard).iter() {
		log::info!("\t{}{}", entry, addr);
	}
}*/