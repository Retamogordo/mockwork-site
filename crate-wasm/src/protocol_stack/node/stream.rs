use futures::channel::mpsc::{unbounded, UnboundedSender, UnboundedReceiver };
use futures::channel::{oneshot};
use futures::{ StreamExt};
use std::sync::{Arc};
use std::fmt::Display;

use super::super::{Chunk, MessageBodyType, ServiceToken, SecondLayerHeader, BlockType};
use super::{ IncomingStreamReceiver, NodeCommandSender};
use super::super::second_layer::{*};
use crate::protocol_stack::second_layer::{ServiceResultType, LineChannelResult};
use crate::{NodeAddress};
use crate::protocol_stack::line::{LineId};
use crate::protocol_stack::node::{NodeCommand};


pub struct StreamHandle {
	node_addr: NodeAddress,
	dest_addr: NodeAddress,
	line_id: LineId,
	token: ServiceToken,
	socket_cmd_tx: Option<NodeCommandSender>,	

    stream_event_rx: UnboundedReceiver<ServiceResultType>,
    stream_data_rx: UnboundedReceiver<MessageBodyType>,
	
    drop_stream_rx: oneshot::Receiver<ChannelError>,
}

impl StreamHandle {
    pub fn token(&self) -> &ServiceToken {
    	&self.token
    }

	fn validate(&mut self) -> Result<(),ChannelError> {
			// Ok means oneshot is not yet Cancelled
			// res not None means oneshot was fired
		match self.drop_stream_rx.try_recv() {
			Ok(res) => if res.is_none() {Ok(())} else {Err(res.unwrap())},
			Err(_) => {
				self.socket_cmd_tx.take();
				Err(ChannelError(
					self.token, 
					LineChannelResult::Disconnected))
			}
		}
	}

	pub fn write(&mut self, msg: MessageBodyType) -> Result<(), ChannelError> {
		self.write_inner(msg, BlockType::Data)
	}

	fn write_inner(&mut self, msg: MessageBodyType, block_type: BlockType) 
													-> Result<(), ChannelError> {
		self.validate()?;

		let block = Chunk::from_message(msg);
		let token = self.token;

		if let Some(tx) = self.socket_cmd_tx.as_mut() {
			let src = self.node_addr;
			let dest = self.dest_addr;

			tx.unbounded_send(NodeCommand(self.line_id,
				SecondLayerCommand::Send(
								block,
								SecondLayerHeader::new(block_type, src, dest),
								Some(token)))
				)
			.map_err(|_err| {ChannelError(token, LineChannelResult::Disconnected)})
			
		} else {
			Err(ChannelError(self.token, LineChannelResult::Disconnected))
		}
	}

	pub async fn read(&mut self) -> Option<MessageBodyType> {
		self.stream_data_rx.next().await
    }

	pub fn cmd(&mut self, cmd: SecondLayerCommand) -> Result<(), ChannelError> {

		self.validate()?;

		if let Some(tx) = self.socket_cmd_tx.as_mut() {
			tx.unbounded_send(NodeCommand(self.line_id, cmd))
			.map_err(|_err| {ChannelError(self.token, LineChannelResult::Disconnected)})
			
		} else {
			Err(ChannelError(self.token, LineChannelResult::Disconnected))
		}
	}

	pub async fn event(&mut self) -> Option<ServiceResultType> {
		self.stream_event_rx.next().await
    }
}

impl Display for StreamHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "[src {}, dst {}, line {}, token {}]", self.node_addr, self.dest_addr, self.line_id, self.token())
	}
}

pub struct ServiceManageHandle {
	node_addr: NodeAddress,
	pub line_id: LineId,
	token: ServiceToken,

	stream_event_tx: UnboundedSender<ServiceResultType>,
	stream_data_tx: UnboundedSender<MessageBodyType>,
	drop_stream_tx: Option<oneshot::Sender<ChannelError>>,
	activate_stream_tx: Option<oneshot::Sender<()>>,
	drop_reason: Option<LineChannelResult>,
}

impl ServiceManageHandle {
	fn new(
		node_addr: NodeAddress,
		line_id: LineId,
		token: ServiceToken,
		stream_event_tx: UnboundedSender<ServiceResultType>,
		stream_data_tx: UnboundedSender<MessageBodyType>,
		activate_stream_tx: oneshot::Sender<()>,
		drop_stream_tx: oneshot::Sender<ChannelError>,

	) -> Self {

		Self {
			node_addr, 
			line_id,
			token,
			stream_event_tx,
			stream_data_tx,
			activate_stream_tx: Some(activate_stream_tx),
			drop_stream_tx: Some(drop_stream_tx),
			drop_reason: Some(LineChannelResult::Unspecified),
		}
	}

	pub fn activate_stream(&mut self) -> Result<(), ChannelError> {
		let tx = self.activate_stream_tx.take();

		if let Some(tx) = tx {
			tx.send(())
				.map_err(|_| ChannelError(self.token, LineChannelResult::Disconnected)) 
		}
		else { 
			log::error!("{} Cannot activate stream", self.node_addr);
			Err(ChannelError(self.token, LineChannelResult::Disconnected)) 
		}
	}

	pub fn activated(&self) -> bool {
		self.activate_stream_tx.is_none()
	}

	pub fn token(&self) -> &ServiceToken {
    	&self.token 
    }

	pub fn send(&mut self, what: ServiceResultType) -> Result<(), ChannelError> {

		match what {
			ServiceResultType::LineChannel(result) => 
				match result {
					LineChannelResult::Data(data) => {
						self.stream_data_tx.unbounded_send(data.take())
							.map_err(|_| ChannelError(self.token, LineChannelResult::Disconnected))
			
					}
					_ => panic!("Invalid data type on stream {}", result),
				},			
			_ => 
				self.stream_event_tx.unbounded_send(what)
					.map_err(|_| ChannelError(self.token, LineChannelResult::Disconnected))
				
		}
	}

	pub fn set_drop_reason(&mut self, reason: LineChannelResult) {
		self.drop_reason = Some(reason);
	}
}

impl Drop for ServiceManageHandle {
	fn drop(&mut self) {
		log::warn!("ServiceManageHandle is dropping stream {} on {}, {}", 
			self.token, self.node_addr, self.drop_reason.as_ref().unwrap());
//		self.drop_stream(ChannelError::ServiceComplete(self.token));
		let _res = self.drop_stream_tx.take().unwrap()
			.send(ChannelError(self.token, self.drop_reason.take().unwrap()));
	}
}

pub fn service_stream(node_addr: NodeAddress, 
	dest_addr: NodeAddress,
	line_id: LineId,
	token: ServiceToken, 
	socket_cmd_tx: NodeCommandSender
) -> (AcquireStreamHandle, ServiceManageHandle) {

	let (stream_event_tx, stream_event_rx): 
		(UnboundedSender<ServiceResultType>, UnboundedReceiver<ServiceResultType>) = unbounded();
	let (stream_data_tx, stream_data_rx): 
		(UnboundedSender<MessageBodyType>, UnboundedReceiver<MessageBodyType>) = unbounded();
	let (activate_stream_tx, activate_stream_rx): 
		(oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
	let (drop_stream_tx, drop_stream_rx):
		(oneshot::Sender<ChannelError>, oneshot::Receiver<ChannelError>) = oneshot::channel();
	let stream_handle = StreamHandle {
		node_addr,
		dest_addr,
		line_id,
		token,
		socket_cmd_tx: Some(socket_cmd_tx),	
	
		stream_data_rx,
		stream_event_rx,
		drop_stream_rx,
	};		

	( AcquireStreamHandle::new(stream_handle, activate_stream_rx),

		ServiceManageHandle::new(
			node_addr,
			line_id,
			token,
			stream_event_tx,
			stream_data_tx,	
			activate_stream_tx,
			drop_stream_tx,
	))
}


impl Drop for StreamHandle {
	fn drop(&mut self) {
		let token = self.token;

		log::warn!("STREAM IS DROPPING {} on {}", token, self.node_addr);
		self.socket_cmd_tx.take()
			.and_then(|tx| {
				Some( tx.unbounded_send( NodeCommand(self.line_id,
					SecondLayerCommand::DropService(token)))
				.map_err(|_err| ChannelError(token, LineChannelResult::Disconnected )))
			});
	}
}

pub struct AcquireStreamHandle {
	stream_handle: StreamHandle,

	activate_stream_rx: oneshot::Receiver<()>,
}

impl AcquireStreamHandle {
	fn new(
		stream_handle: StreamHandle,
		activate_stream_rx: oneshot::Receiver<()>) -> Self {
		Self {
			stream_handle,
			activate_stream_rx 
		}
	}
	
	pub async fn acquire(mut self) -> Result<StreamHandle, ChannelError> {
		match self.activate_stream_rx.await {
			Ok(()) => {
				Ok(self.stream_handle)
			},
			Err(_) => {
				self.stream_handle.validate()?; // try to get error from drop reason
				Err(ChannelError(*self.stream_handle.token(), LineChannelResult::Disconnected))
			}
		}
	}

	pub fn yield_stream(self) -> StreamHandle {
		self.stream_handle
	}
}

pub struct IncomingHandle {
	incoming_rx: Arc<async_std::sync::Mutex<IncomingStreamReceiver>>,
}

impl IncomingHandle {
	pub fn new(
			incoming_rx: Arc<async_std::sync::Mutex<IncomingStreamReceiver>>) -> Self {
		Self {
			incoming_rx 
		}
	}
	
	pub async fn acquire(&self) -> Option<StreamHandle> {
		let mut guard = self.incoming_rx.lock().await;

		while let Some(mut handle) = guard.next().await {
			if let Ok(()) = handle.validate() {
				return Some(handle);
			}
		}
		None			
	}
}

pub struct ChannelError(pub ServiceToken, pub LineChannelResult);

impl ChannelError {
	pub fn token(&self) -> &ServiceToken {
		&self.0
	}
}

impl Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "({}->{})", self.0, self.1)
	}
}
