use std::sync::{Arc, RwLock};
//use core::time::Duration;
use std::collections::{HashMap,     };
use crate::pipe::{EventTarget, SharedPipeAlloc};
use crate::protocol_stack::line::{LineId};
use super::node::{NodeAddress, AddressMapEntry};
use crate::protocol_stack::second_layer::{SecondLayerCommand, SecondLayerEvent, LineChannelResult,
    ServiceRequestType, ServiceResultType};
use crate::protocol_stack::{Chunk, SecondLayerHeader, BlockType};
use crate::protocol_stack::{ServiceToken};
use crate::protocol_stack::layer_pipe_item::{*};

//use colored::*;

struct ChannelJoint(Option<LineId>, Option<LineId>);

impl ChannelJoint {
    fn other(&self, this: LineId) -> Option<&LineId> {

        if Some(this) == self.0 {self.1.as_ref()} 
        else if Some(this) == self.1 {self.0.as_ref()} else {None}
    }
}

impl std::fmt::Display for ChannelJoint {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, ">{}--{}<", self.0.unwrap_or_default(), 
            self.1.unwrap_or_default())
    }
}


struct PeerChannel{
    node_addr: NodeAddress,
    src_addr: Option<NodeAddress>,
    channel_joint: ChannelJoint,
    dup_channel_req: bool,
}

impl PeerChannel {
    pub fn from_origin(
        node_addr: NodeAddress,
        dest_line_id: LineId,
    ) -> Self {
        Self {
            node_addr,
            src_addr: None,
            channel_joint: ChannelJoint(None, Some(dest_line_id)),
            dup_channel_req: false,
        }
    }

    pub fn from_incoming(
        node_addr: NodeAddress,
        src_addr: Option<NodeAddress>,
        source_line_id: LineId,
    ) -> Self {
        Self {
            node_addr,
            src_addr,
            channel_joint: ChannelJoint(Some(source_line_id), None),
            dup_channel_req: false,
        }
    }

    fn is_origin(&self) -> bool {
        self.channel_joint.0.is_none()
    }

    fn is_dest(&self) -> bool {
        !self.src_addr.is_none()
    }


    fn on_first_layer_channel_established(&mut self, 
        service_token: ServiceToken, 
        dest_line_id: LineId,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {
//        log::info!("----------- Established peer channel on {}, {}", self.node_addr, service_token);
        if self.is_dest() {
            
            self.channel_joint.0 = Some(dest_line_id);

            shared_alloc.replace_propagate_current(
                NodeEvent(*self.channel_joint.0.as_ref().unwrap(),
                    SecondLayerEvent::IncomingPeerChannel(
                        self.src_addr.unwrap(),
                        service_token)
                ).into());
        }
    }

    fn on_incoming_first_layer_channel(&mut self, 
        service_token: &ServiceToken, 
        dest_line_id: LineId,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {
        if  self.dup_channel_req {
            log::debug!("\t\t\tDUPLICATE CHANNEL REQUEST AT {}, token {}", self.node_addr, service_token);
            return;
        }
        self.dup_channel_req = true;
        self.channel_joint.1 = Some(dest_line_id);

        // source_line_id None means this is the origin of request
        if self.is_origin() {
            shared_alloc.replace_propagate_current(
                NodeEvent(
                    dest_line_id,
                    SecondLayerEvent::ServiceStarted(service_token.clone())).into()
            );
        }
        else {

        shared_alloc.replace_propagate_current(
            NodeCommand(*self.channel_joint.0.as_ref().unwrap(),
                    SecondLayerCommand::ServiceRequest(
                        service_token.clone(),
                        ServiceRequestType::LineChannel(false)))
                .into());
        }
    }

    fn on_drop(&self, service_token: ServiceToken, 
        line_id: LineId,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        if self.is_origin() || self.is_dest() {
//            shared_alloc.push(event.into());
        } else { 
            if let Some(other_joint_end) = self.channel_joint.other(line_id) {
                log::debug!("{} dropping {}", self.node_addr, service_token);
                shared_alloc.replace_propagate_current(
                    NodeCommand(*other_joint_end,
                    SecondLayerCommand::DropService(service_token)).into()
                );
            } 
        }
    }

    fn keep_alive(&self, 
        line_id: LineId,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        if self.is_origin() || self.is_dest() {
//            shared_alloc.push(event.into());
        } else { 
            if let Some(other_joint_end) = self.channel_joint.other(line_id) {
//                log::info!("{} peer keeping alive {}", self.node_addr, service_token);
                shared_alloc.replace_propagate_current(
                    NodeCommand(*other_joint_end,
                    SecondLayerCommand::CheckExpiredTokens).into()
                );
            } 
        }
    }

}

pub struct PeerChannelMonitor {
    node_addr: NodeAddress,
    active_channels: HashMap<ServiceToken, PeerChannel>,
    waiting_channels: HashMap<ServiceToken, PeerChannel>,
    address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,
}

impl PeerChannelMonitor {

    pub fn new(node_addr: NodeAddress, address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>) -> Self {
        Self {
            node_addr,
            address_map,
            active_channels: HashMap::new(),
            waiting_channels: HashMap::new(),
        }
    }

    fn on_channel_request(&mut self, 
        service_token: ServiceToken, 
        dest_addr: NodeAddress,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {
        
        if let Some(dest_line_entry) = self.address_map.read().unwrap()
            .get(&dest_addr) {
            let node_addr = self.node_addr;

            if !self.waiting_channels.contains_key(&service_token) {

                self.waiting_channels.insert(service_token, PeerChannel::from_origin(
                    node_addr,
                    dest_line_entry.line_id
                )); 

                shared_alloc.replace_propagate_current(
                    Self::make_req_cmd(node_addr, service_token, dest_line_entry.line_id, dest_addr).into(),
                );
            }
        }                            
    }

    fn make_req_cmd(node_addr: NodeAddress, service_token: ServiceToken, line_id: LineId, dest_addr: NodeAddress) -> NodeCommand {
        let hdr = SecondLayerHeader::new( 
            BlockType::ChannelRequest,
            node_addr,
            dest_addr,
        );

        let msg = Chunk::default();

        NodeCommand(line_id, SecondLayerCommand::Send(msg, hdr, Some(service_token)))     
    }

    fn on_foreign_request(&mut self, 
        service_token: ServiceToken,
        hdr: SecondLayerHeader,
        source_line_id: LineId,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {

            log::debug!("----------- on_foreign_request: {}", self.node_addr);
        if hdr.dest == self.node_addr {
            shared_alloc.replace_propagate_current(
                NodeCommand(source_line_id,
                    SecondLayerCommand::ServiceRequest(
                        service_token,
                        ServiceRequestType::LineChannel(false))
                ).into()
            );
        }
        else {
            if let Some(dest_entry) = self.address_map.read().unwrap()
                .get(&hdr.dest) {

                let msg = Chunk::default();
                    
                shared_alloc.take_current();
                shared_alloc.push_done(
                    NodeCommand(dest_entry.line_id,
                        SecondLayerCommand::Send(msg,
                            hdr.clone(),
                            Some(service_token)) // ???
                    ).into()
                );
            }
        }
    }
    
}

impl EventTarget<LayerPipeItem> for PeerChannelMonitor {
	fn notify(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        match shared_alloc.current() {
            LayerPipeItem::SomeEvent(line_id, LayerEvent::SecondLayer(ref event)) => 
            match event {
                SecondLayerEvent::ServiceMessage(service_token, 
                    ServiceResultType::SomeService(ref msg)
                ) => {
                //                log::info!("block recv in peer ch");
                let hdr: &SecondLayerHeader = msg.header().into();
                let source_line_id = line_id;

    //               if let Some(service_token) = hdr.service_token {
                match hdr.block_type { 

                    BlockType::ChannelRequest => {
                        let node_addr = self.node_addr;
                        let dest_addr = hdr.dest;
                        let src_addr 
                            = if node_addr == dest_addr {Some(hdr.src)} else {None};

                        if !self.waiting_channels.contains_key(&service_token) { 
                            let service_token = service_token.clone();
                            let source_line_id = *source_line_id;

                            self.waiting_channels.insert(service_token,
                                PeerChannel::from_incoming(
                                    node_addr,
                                    src_addr,
                                    source_line_id
                            ));
                            self.on_foreign_request(service_token, hdr.clone(), source_line_id, shared_alloc);
                        } 
                    },
                    _ => (), //shared_alloc.push(event.into()),
                }
    //                }
            },
    
            SecondLayerEvent::ServiceMessage(service_token, ref res) => {
                    //                    log::info!("{} in peer ch", service_token);
                if let Some(channel) = self.active_channels.get(&service_token) {

                    if let Some(other_joint_end) = channel.channel_joint.other(*line_id) {
                        if let Some(msg) = res.data() {
                            let service_token = service_token.clone();
                            let msg = msg.clone();
        
                            shared_alloc.push_done(
                                LayerPipeItem::SomeCommand(*other_joint_end,
                                    LayerCommand::SecondLayer(
                                    SecondLayerCommand::SendAsIs(msg, Some(service_token)))
                                )
                            );
                            shared_alloc.take_current();
                        }
                    } else {
                        let event = shared_alloc.take_current().unwrap();
                        shared_alloc.push_done(event);
                    }
                }
            }

            SecondLayerEvent::ServiceStarted(service_token) => {

                if let Some(mut channel) = self.waiting_channels.remove(&service_token) {
    //                       if let Some(modified_items) =
                    if !self.active_channels.contains_key(&service_token) {

                        let service_token = *service_token;
                        let dest_line_id = *line_id;

                        channel.on_first_layer_channel_established(
                            service_token, 
                            dest_line_id,
                            shared_alloc
                        ); 

                        self.active_channels.insert(service_token.clone(), channel);
                    }
                }
            },
            SecondLayerEvent::IncomingDataChannel(service_token) => {
//                        log::info!("{} IncomingDataChannel", self.node_addr);
                if let Some(mut channel) = self.waiting_channels.remove(&service_token) {
                    let service_token = service_token.clone();
                    if !self.active_channels.contains_key(&service_token) {
                
                        let dest_line_id = *line_id;

                        channel.on_incoming_first_layer_channel(
                            &service_token, dest_line_id, shared_alloc);
                    }

                    self.active_channels.insert(service_token, channel);
                }
            },
    
            SecondLayerEvent::ServiceComplete(service_token, _) => {
                    let dest_line_id = *line_id;
                    self.waiting_channels.remove(&service_token);
                    if let Some(channel) = self.active_channels.remove(&service_token) {                                             
                        channel.on_drop(*service_token, dest_line_id, shared_alloc);
                    }
            },

            SecondLayerEvent::FirstLayerChannelKeptAlive(service_token) => {
                let dest_line_id = *line_id;
                if let Some(channel) = self.active_channels.get(&service_token) {                                             
                    channel.keep_alive(dest_line_id, shared_alloc);
                }
            }
            
            _=> (), // shared_alloc.push(event.into()),
            },
            
            LayerPipeItem::SomeCommand(_, LayerCommand::SecondLayer(ref cmd)) => 
                match cmd {
                    SecondLayerCommand::ServiceRequest(
                        service_token,
                        ServiceRequestType::PeerChannel(dest)
                    ) => {
                        self.on_channel_request(*service_token, *dest, shared_alloc);
                    },
                    SecondLayerCommand::CheckExpiredTokens => {
                        // only check for waiting token, as active tokens are controlled
                        // by first layer channel monitor
                        self.waiting_channels.retain(|token, _| {
                            if token.expired() {
                                
                                shared_alloc.push(
                                    SecondLayerEvent::ServiceComplete(
                                        *token, 
                                        ServiceResultType::LineChannel(LineChannelResult::Expired)).wrap().into());
                                false
                            } else { true }
                        });
                    }                        
                    _ => (), //shared_alloc.push(cmd.into()),
                },
                
            _ => unimplemented!(),
        };
    }
}
