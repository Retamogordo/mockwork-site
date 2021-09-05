use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::cell::RefCell;
use std::collections::VecDeque;
use super::physical_layer::*;
use super::first_layer::*;
use super::second_layer::{*};
use super::channel_monitor::{ChannelMonitor};
use super::address_map_resolver::{AddressMapResolver};
use super::peer_channel::PeerChannelMonitor;
use super::expired_streams_cleaner::ExpiredStreamsCleaner;
use crate::pipe::Pipe;
use super::line::{LineModel, LineId};
use super::node::{AddressMapEntry, NodeAddress};
use crate::utils::TakeIter;
use super::layer_pipe_item::{*};


pub struct PhysicalLayerProtocolStack {
	pipe: Pipe<LayerPipeItem>,
}

impl PhysicalLayerProtocolStack {
	pub fn new(line_model: LineModel) -> Self {
		let pipe = Pipe::new();

		let ping = LinePing::new(line_model);
//		pipe.hook(ping);

		Self { pipe }
	}
	
	fn propagate_items(&mut self, 
		items: impl Iterator<Item = (LayerPipeItem, bool)>) 
			-> (&mut VecDeque<(LayerPipeItem, bool)>, 
				&mut VecDeque<(LayerPipeItem, bool)>) {

			let (cmds, evs) = self.pipe.send(items);
			for ev in evs.iter_mut() {
				ev.0.to_upper();
			} 
			(cmds, evs)
	}

}

pub struct FirstLayerProtocolStack {
	pipe: Pipe<LayerPipeItem>,
//	pipe: Pipe<FirstLayerItems>,
}

impl FirstLayerProtocolStack {
	pub fn new(node_addr: NodeAddress, 
		line_model: LineModel, 
		max_channels: usize,
	) -> Self {
		let mut pipe = Pipe::new();
		
//		pipe.hook(LineStats::new(line_model.clone()));
		pipe.hook(ChannelMonitor::new(node_addr, line_model.clone(), max_channels));
		
		pipe.hook(DefaultChunkSender::new());

		Self { pipe }
	}
	
	fn propagate_items(&mut self, 
		items: impl Iterator<Item = (LayerPipeItem, bool)>) 
		-> (&mut VecDeque<(LayerPipeItem, bool)>, 
			&mut VecDeque<(LayerPipeItem, bool)>) {


		let (cmds, evs) = self.pipe.send(items);
		for cmd in cmds.iter_mut() {
			cmd.0.to_lower();
		} 
		for ev in evs.iter_mut() {
			ev.0.to_upper();
		} 
		(cmds, evs)
	}
}

pub struct SecondLayerProtocolStack {
//	pipe: Pipe<SecondLayerItems>,
	pipe: Pipe<LayerPipeItem>,
}

impl SecondLayerProtocolStack {
	pub fn new() -> Self {
		let pipe = Pipe::new();

//		pipe.hook(DefaultBlockSender::new());

		Self { pipe }
	}

	fn propagate_items(&mut self, 
		items: impl Iterator<Item = (LayerPipeItem, bool)>) 
		-> (&mut VecDeque<(LayerPipeItem, bool)>, 
			&mut VecDeque<(LayerPipeItem, bool)>) {

		let (cmds, evs) = self.pipe.send(items);
		for cmd in cmds.iter_mut() {
			cmd.0.to_lower();
		} 
		(cmds, evs)
	}

}

pub struct ProtocolStack {
//	line_id: LineId,
	layer0: RefCell<PhysicalLayerProtocolStack>,
	layer1: RefCell<FirstLayerProtocolStack>,
	layer2: RefCell<SecondLayerProtocolStack>,
}

impl ProtocolStack {
	pub fn new(
		node_addr: NodeAddress,
		line_id: LineId,
		line_model: LineModel,
		max_first_layer_channels: usize,
	) -> Self {
		Self {
//			line_id,
			layer0: RefCell::new(PhysicalLayerProtocolStack::new(line_model.clone())),
			layer1: RefCell::new(FirstLayerProtocolStack::new(node_addr, line_model, max_first_layer_channels)),
			layer2: RefCell::new(SecondLayerProtocolStack::new()),
		}
	}

	pub fn propagate_command(&mut self, 
		cmd: SecondLayerCommand,
		res_cmds: &mut Vec<PhysicalLayerCommand>,
		res_events: &mut Vec<SecondLayerEvent>,
	) {

		let mut layer2 = self.layer2.borrow_mut();
		let (cmds, evs) = layer2.propagate_items(TakeIter::new((cmd.wrap().into(), false)));

		res_events.extend(
			evs.drain(..).map(|(item, _)| item.as_event().unwrap().into()));

		let mut layer1 = self.layer1.borrow_mut();
		let (cmds, evs) = layer1.propagate_items(cmds.drain(..));

		let (_, evs) = layer2.propagate_items(evs.drain(..));
		res_events.extend(
			evs.drain(..).map(|(item, _)| item.as_event().unwrap().into()));

		let mut layer0 = self.layer0.borrow_mut();
		let (cmds, evs) = layer0.propagate_items(cmds.drain(..));
		
		let (_, evs) = layer1.propagate_items(evs.drain(..));

		let (_, evs) = layer2.propagate_items(evs.drain(..));

		res_events.extend(
			evs.drain(..).map(|(item, _)| item.as_event().unwrap().into()));
		res_cmds.extend(
			cmds.drain(..).map(|(item, _)| item.as_cmd().unwrap().into())
		);

		log::debug!("in protocol layers: {}", res_cmds.len());
	}

	pub fn propagate_event(&mut self, 
		event: PhysicalLayerEvent,
		res_cmds: &mut Vec<PhysicalLayerCommand>,
		res_events: &mut Vec<SecondLayerEvent>,
	) {
//		res_cmds.clear();

		let mut layer0 = self.layer0.borrow_mut();
		let (cmds, evs) = layer0.propagate_items(TakeIter::new((event.wrap().into(), false)));

		res_cmds.extend(
			cmds.drain(..).map(|(item, _)| item.as_cmd().unwrap().into()));

		let mut layer1 = self.layer1.borrow_mut();
		let (cmds, evs) = layer1.propagate_items(evs.drain(..));

		let (cmds, _) = layer0.propagate_items(cmds.drain(..));

		res_cmds.extend(
			cmds.drain(..).map(|(item, _)| item.as_cmd().unwrap().into()));

		let mut layer2 = self.layer2.borrow_mut();
		let (cmds, evs) = layer2.propagate_items(evs.drain(..));		

		let (cmds, _) = layer1.propagate_items(cmds.drain(..));

		let (cmds, _) = layer0.propagate_items(cmds.drain(..));
		
		res_cmds.extend(
			cmds.drain(..).map(|(item, _)| item.as_cmd().unwrap().into()));
		res_events.extend(
			evs.drain(..).map(|(item, _)| item.as_event().unwrap().into()));
	}

}
// ----------------------------------------------------------------------------------
pub struct NodeLayer {
	pipe: Pipe<LayerPipeItem>,
}
	
impl NodeLayer {
	pub fn new(
		node_addr: NodeAddress, 
		lines: Arc<Vec<LineId>>,
	    address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,

	) -> Self {
		let mut pipe = Pipe::new();

		pipe.hook(ExpiredStreamsCleaner::new(node_addr, Arc::clone(&lines)));
		pipe.hook(AddressMapResolver::new(node_addr, Arc::clone(&lines), Arc::clone(&address_map)));
		pipe.hook(PeerChannelMonitor::new(node_addr, Arc::clone(&address_map)));
		Self { pipe }
	}

	fn propagate_items(&mut self, 
		items: impl Iterator<Item = (LayerPipeItem, bool)>) 
			-> (&mut VecDeque<(LayerPipeItem, bool)>, 
				&mut VecDeque<(LayerPipeItem, bool)>) {

		self.pipe.send(items)
	}

}
	
pub struct NodeProtocolStack {
	layer3: NodeLayer,
}
	
impl NodeProtocolStack {
	pub fn new(node_addr: NodeAddress, 
		lines: Arc<Vec<LineId>>,
	    address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,
	) -> Self {
		let layer3 = NodeLayer::new(
			node_addr, 
			lines, 
			address_map,
		);
		Self { 
			layer3,
		}

	}

	pub fn propagate_command(&mut self, cmd: NodeCommand, 
		res_cmds: &mut Vec<LayerPipeItem>,
		res_events: &mut Vec<LayerPipeItem>) { 
		
		let (cmds, evs) = self.layer3.propagate_items(TakeIter::new((cmd.into(), false)));
		
		res_events.extend(evs.drain(..).map(|(item, _)| item));
		res_cmds.extend(cmds.drain(..).map(|(item, _)| item));
	}

	pub fn propagate_event(&mut self, event: NodeEvent, 
		res_cmds: &mut Vec<LayerPipeItem>,
		res_events: &mut Vec<LayerPipeItem>) {

		let (cmds, evs) = self.layer3.propagate_items(TakeIter::new((event.into(), false)));
	
		res_events.extend(evs.drain(..).map(|(item, _)| item));
		res_cmds.extend(cmds.drain(..).map(|(item, _)| item));
	}
}
