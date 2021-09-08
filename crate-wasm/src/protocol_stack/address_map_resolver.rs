use std::sync::{Arc, RwLock};
use instant::Instant;
use std::collections::HashMap;
use crate::pipe::{EventTarget,SharedPipeAlloc};
use crate::protocol_stack::line::{LineId};
use super::node::{NodeAddress, AddressMapEntry};
use crate::protocol_stack::second_layer::{SecondLayerCommand, SecondLayerEvent, 
    ServiceRequestType, ServiceResultType};
use crate::protocol_stack::{Chunk, SecondLayerHeader, BlockType, MessageBodyType};
use crate::protocol_stack::{ServiceToken};
use super::layer_pipe_item::{*};

pub struct AddressMapResolver {
    node_addr: NodeAddress,
    lines: Arc<Vec<LineId>>,
    service_entries: HashMap<ServiceToken, (LineId, NodeAddress)>,
    address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,
}

impl AddressMapResolver {
    pub fn new(
        node_addr: NodeAddress, 
        lines: Arc<Vec<LineId>>,
        address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,
    ) -> Self {
        Self { 
            node_addr,
            lines,
            service_entries: HashMap::new(),
            address_map: Arc::clone(&address_map),
        }
    }

    fn on_start_service(&mut self, 
        service_token: ServiceToken, 
        dest: NodeAddress,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {
//            log::info!("start addr. map srvc {} -> {}", self.node_addr, dest);
        if dest == NodeAddress::ANY {
            self.address_map.write().unwrap().clear();
        } else {
            self.address_map.write().unwrap().remove(&dest);
        }

        if !self.service_entries.contains_key(&service_token) && !service_token.expired() {
            self.service_entries.insert(service_token, (LineId::default(), dest));
        }

        shared_alloc.extend( self.lines.iter()
            .map(|&line_id| {
                self.make_req_cmd(service_token, line_id, dest).into()
            }));
        shared_alloc.take_current();

        shared_alloc.push_done( 
            NodeEvent(LineId::default(),
                SecondLayerEvent::ServiceStarted(service_token)).into()
            );
    }

    fn on_drop_service(&mut self, service_token: ServiceToken,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {
        if let Some(_) = self.service_entries.remove(&service_token) {
            shared_alloc.extend( self.lines.iter()
                .map(|&line_id| self.make_drop_cmd(service_token, line_id).into()));
            shared_alloc.take_current();
        }
    }

    fn make_req_cmd(&self, service_token: ServiceToken, line_id: LineId, dest: NodeAddress) -> NodeCommand {
        let hdr = SecondLayerHeader::new( 
            BlockType::AddressMapRequest,
            self.node_addr,
            dest,
        );

        let msg = Chunk::from_message(
            MessageBodyType::TimeStamp(Instant::now()));

        NodeCommand(line_id, SecondLayerCommand::Send(msg, hdr, Some(service_token)))     
    }

    fn make_resp_cmd(&self, service_token: ServiceToken, line_id: LineId, dest: NodeAddress) -> NodeCommand {
        let hdr = SecondLayerHeader::new( 
            BlockType::AddressMapResponse,
            self.node_addr,
            dest,
        );

        let msg = Chunk::from_message(
            MessageBodyType::TimeStamp(Instant::now()));

        NodeCommand(line_id, SecondLayerCommand::Send(msg, hdr, Some(service_token)))  
    }

    fn make_drop_cmd(&self, service_token: ServiceToken, line_id: LineId)  -> NodeCommand {
        let hdr = SecondLayerHeader::new( 
            BlockType::DropToken,
            self.node_addr,
            NodeAddress::ANY,
        );

        let msg = Chunk::default();

        NodeCommand(line_id, SecondLayerCommand::Send(msg, hdr, Some(service_token)))
    }

    fn update_own_map_on_req(&self, msg: &Chunk, source_line_id: LineId, hdr: &SecondLayerHeader) {
        let delay =
            match msg.body() {
                MessageBodyType::TimeStamp(ts) => ts.elapsed(),
                _ => unreachable!(),
            };

        let new_entry =  AddressMapEntry { 
            line_id: source_line_id, 
            oneway_delay: delay
        };
        self.address_map.write().unwrap()
            .entry(hdr.src)
            .and_modify(|entry| {
                if entry.oneway_delay > delay { *entry = new_entry }
            })
            .or_insert(new_entry);
    }
    
    fn update_map_on_resp(&self, msg: &Chunk, source_line_id: LineId, hdr: &SecondLayerHeader) {
        let delay =
            match msg.body() {
                MessageBodyType::TimeStamp(ts) => ts.elapsed(),
                _ => unreachable!(),
            };
        let new_entry =  AddressMapEntry { 
            line_id: source_line_id, 
            oneway_delay: delay
        };
        
        self.address_map.write().unwrap()
            .entry(hdr.src)
            .and_modify(|entry| {
                if entry.oneway_delay > delay { *entry = new_entry }
            })
            .or_insert(new_entry);
    }
    
    fn update_map_passively(&self, source_line_id: LineId, hdr: &SecondLayerHeader) {
        if self.node_addr != hdr.src {  // do not register self address
            let delay = core::time::Duration::from_secs(1_000_000);
            let new_entry =  AddressMapEntry { 
                line_id: source_line_id, 
                oneway_delay: delay
            };
            self.address_map.write().unwrap()
                .entry(hdr.src)
                .or_insert(new_entry);
        }
    }
}

impl EventTarget<LayerPipeItem> for AddressMapResolver {

	fn notify(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        match shared_alloc.current() {
            LayerPipeItem::SomeEvent(source_line_id, LayerEvent::SecondLayer(event)) => 
            match event {
                SecondLayerEvent::ServiceMessage(service_token, 
                    ServiceResultType::SomeService(ref msg)
                ) => {
                let hdr: &SecondLayerHeader = msg.header().into();
                    
                match hdr.block_type { 
                    BlockType::AddressMapRequest => {

                        if service_token.expired() || self.service_entries.contains_key(service_token) {
                            shared_alloc.take_current();
                            return;
                        }
                        let msg = msg.clone();
                        let service_token = service_token.clone();
                        let source_line_id = *source_line_id;

                        self.update_own_map_on_req(&msg, source_line_id, &hdr);

                        if hdr.dest != self.node_addr {
                            self.service_entries.insert(service_token, 
                                (source_line_id, hdr.dest));

                            let hdr = hdr.clone();
                            shared_alloc.extend(
                                self.lines.iter()
                                    .filter(|&line| *line != source_line_id)
                                    .map(|&line_id| {                                            
                                        NodeCommand(line_id,
                                            SecondLayerCommand::Send(msg.clone(),
                                                hdr,
                                                Some(service_token)
                                            )                                            
                                    ).into()
                            }));

                            if hdr.dest == NodeAddress::ANY {
                                shared_alloc.push_done(
                                    self.make_resp_cmd(service_token, source_line_id, hdr.src)
                                        .into()
                                );   
                            }
                        }
                        else {
                            let hdr = hdr.clone();
                            shared_alloc.push_done(
                                self.make_resp_cmd(service_token, source_line_id, hdr.src)
                                    .into()
                            );   
                        }
                        shared_alloc.take_current();
                    },

                    BlockType::AddressMapResponse => {
                        self.update_map_on_resp(msg, *source_line_id, &hdr);

                        let msg = msg.clone();
                        let service_token = service_token.clone();
                        
                        if let Some((req_line_id, dest)) = self.service_entries.get(&service_token) {

                            if self.node_addr == hdr.dest {
                                // for non broadcast request can finish service
                                if *dest != NodeAddress::ANY {
                                    let src = hdr.src;
                                    shared_alloc.push_done(
                                        NodeEvent(LineId::default(),
                                            SecondLayerEvent::ServiceComplete(
                                                service_token,
                                                ServiceResultType::AddressMapUpdate(src))
                                        ).into()
                                    );
                                    self.service_entries.remove(&service_token);
                                    shared_alloc.take_current();
                                }
                            } else {   
                                shared_alloc.push_done(
                                    NodeCommand(*req_line_id,
                                        SecondLayerCommand::SendAsIs(msg, Some(service_token))
                                    ).into());
                                shared_alloc.take_current();
                            } 
                        }
                    },
                    BlockType::DropToken => {
                        let msg = msg.clone();
                        let service_token = service_token.clone();
                        let source_line_id = *source_line_id;

                        if let Some(_) = self.service_entries.remove(&service_token) {
                            log::debug!("drop addr token {} recv on {}", 
                                service_token,self.node_addr);
                            shared_alloc.extend(
                                self.lines.iter()
                                    .filter(|&line| *line != source_line_id)
                                    .map(|&line_id| {                                            
                                        NodeCommand(line_id,SecondLayerCommand::SendAsIs(msg.clone(),
                                            Some(service_token))                                            
                                ).into()
                            }));
                        } else {
                            self.update_map_passively(source_line_id, &hdr);
                        }
                    },
                    _ => {
                        self.update_map_passively(*source_line_id, &hdr);
                    },
                }
                    
                },
                _ => (),
            },
            LayerPipeItem::SomeCommand(_, LayerCommand::SecondLayer(cmd)) => {
                match cmd {
                    SecondLayerCommand::ServiceRequest(
                        service_token,
                        ServiceRequestType::AddressMap(dest)) => {
                            self.on_start_service(*service_token, *dest, shared_alloc);
                        },

                    SecondLayerCommand::DropService(service_token) => {
                        self.on_drop_service(*service_token, shared_alloc);
                    },

                    SecondLayerCommand::CheckExpiredTokens => {
                        self.service_entries.retain(|token, _| { 
                            !token.expired()
                        });
                    },

                    _ => (), //shared_alloc.propagate_current(),
                };        
            },
            _ => unimplemented!(),
        }
    }
}

