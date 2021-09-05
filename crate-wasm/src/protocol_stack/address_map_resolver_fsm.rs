use std::sync::{Arc, RwLock};
//use core::time::Duration;
use instant::Instant;
use std::collections::HashMap;
use crate::pipe::{EventTarget, SharedPipeAlloc};
//use crate::pipe::{PipeEntities, EventTarget,SharedPipeAlloc};
use crate::protocol_stack::line::{LineId};
use super::node::{NodeAddress, AddressMapEntry};
use crate::protocol_stack::second_layer::{SecondLayerCommand, SecondLayerEvent, 
    ServiceRequestType, ServiceResultType};
use crate::protocol_stack::{Chunk, SecondLayerHeader, BlockType, MessageBodyType};
use crate::protocol_stack::{ServiceToken};
use super::layer_pipe_item::{*};
use crate::fsm::{FSM, TransitionHandle, SignalIdentifier, SignalInput};
use strum_macros::{Display};

#[derive (PartialEq, Eq, Hash, Clone, Copy)]
enum SignalsEnum {
	RequestSignal,
    ForeignRequestSignal,
	DropSignal,
    ServiceTokenExpiredSignal,
    DestRespondSignal,
    ResponseReceivedSignal,
    SourceResponseReceivedSignal,
}
#[derive(Clone)]
struct SignalInputData {
    service_token: ServiceToken,
//    dest_addr: Option<NodeAddress>, 
    source_line_id: Option<LineId>,
    hdr: Option<SecondLayerHeader>,
    msg: Option<Chunk>,
}

struct FSMSignals(SignalsEnum, <Self as SignalInput>::Item);

impl FSMSignals {
 /*   fn s: SignalsEnum) -> Self {
        Self(s, None)
    }*/
}
impl SignalInput for FSMSignals {
	type Item = SignalInputData;
	fn get(&self) -> &Self::Item {
		&self.1
	}
	fn set(&mut self, data: Self::Item) {
		self.1 = data;
	}
}

impl SignalIdentifier for FSMSignals {
    type IdType = SignalsEnum;
    fn id(&self) -> Self::IdType {
        self.0
    }
}

impl PartialEq for FSMSignals {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for FSMSignals {
}

impl std::hash::Hash for FSMSignals {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[derive (PartialEq, Clone, Copy, Display)]
enum FSMStates {
	ActiveState,
}

type SignalInputItem = <FSMSignals as SignalInput>::Item;

struct FSMStateData {
    node_addr: NodeAddress,
    service_token: Option<ServiceToken>,
    source_req_line: Option<LineId>,
//    req_broadcast_instant: Option<Instant>,
    dest_addr: NodeAddress,
//    hdr: Option<SecondLayerHeader>,
    lines: Option<Arc<Vec<LineId>>>,
    address_map: Option<Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>>,
	modify_pipe_items: Option<fn( &mut Self, &SignalInputItem, &mut SharedPipeAlloc<LayerPipeItem> )>,
}

impl FSMStateData {
    fn modify_pipe_items(
        &mut self, 
        input: &SignalInputItem,
        alloc: &mut SharedPipeAlloc<LayerPipeItem> ) {
            if let Some(closure) = self.modify_pipe_items.take() {
                (closure)(self, input, alloc);
            }
    }
}

impl Default for FSMStateData {
    fn default() -> Self {
        Self {
            node_addr: NodeAddress::default(),
            service_token: None,
            source_req_line: None,
            dest_addr: NodeAddress::default(),
            lines: None,
            address_map: None,
            modify_pipe_items: None,
        }
    }
}

struct OnRequest; 
impl TransitionHandle for OnRequest {
	type DataType = FSMStateData;
	type SignalType = FSMSignals;

	fn call(&self, input: &<Self::SignalType as SignalInput>::Item, 
        data: &mut Self::DataType) -> bool {
//        log::info!("Address map service on_start_service {} -> {}", input.0, self.dest_addr);
//        let service_token = input.service_token;

        if input.service_token.expired() {
            return false;
        }

        data.service_token = Some(input.service_token);

        let msg = input.msg.as_ref().unwrap();
        let hdr: SecondLayerHeader = input.hdr.unwrap();
        data.dest_addr = hdr.dest;

//        data.req_broadcast_instant = Some(Instant::now());

        if data.dest_addr == NodeAddress::ANY {
            data.address_map.as_ref().unwrap().write().unwrap().clear();
        } else {
            data.address_map.as_ref().unwrap().write().unwrap().remove(&data.dest_addr);
        }

        data.modify_pipe_items = Some( |data, input, alloc| {
            let msg = input.msg.as_ref().unwrap();
            let hdr: SecondLayerHeader = input.hdr.unwrap();

            alloc.extend(data.lines.as_ref().unwrap().iter()
                .map(|&line_id| {
        log::info!("Address map service sending on line {}", line_id);
          
                    NodeCommand(line_id, SecondLayerCommand::Send(msg.clone(), hdr.clone(), data.service_token)).into()     
            }));
			alloc.take_current();

			alloc.push_done(	
                NodeEvent(LineId::default(),
                SecondLayerEvent::ServiceStarted(data.service_token.unwrap())).into()
            );
		});
        true
    }
}

struct OnForeignRequest; 
impl OnForeignRequest {
    fn update_own_map_on_req(&self, src: NodeAddress, 
        input: &<<Self as TransitionHandle>::SignalType as SignalInput>::Item,
        data: &mut <Self as TransitionHandle>::DataType) {

        let delay =
            match input.msg.as_ref().unwrap().body() {
                MessageBodyType::TimeStamp(ts) => ts.elapsed(),
                _ => unreachable!(),
        };
        let new_entry =  AddressMapEntry { 
            line_id: input.source_line_id.unwrap(), 
            oneway_delay: delay
        };
        data.address_map.as_ref().unwrap().write().unwrap()
            .entry(src)
            .and_modify(|entry| {
                if entry.oneway_delay > delay { *entry = new_entry }
            })
            .or_insert(new_entry);
    }
}

impl TransitionHandle for OnForeignRequest {
	type DataType = FSMStateData;
	type SignalType = FSMSignals;

	fn call(&self, input: &<Self::SignalType as SignalInput>::Item, 
        data: &mut Self::DataType) -> bool {

        let service_token = input.service_token;
        let msg = input.msg.as_ref().unwrap();
        let hdr: &SecondLayerHeader = msg.header().into();

        self.update_own_map_on_req(hdr.src, input, data);
    
//        log::info!("\t\t\tAddress map at {} got foreign req from {}, before token check ", 
//            data.node_addr, hdr.src);

        if let Some(prev_token) = data.service_token {
            if prev_token.id() > service_token.id() {
                return false;
            }
        }

        log::info!("\t\t\tAddress map at {} got foreign req from {}, after token check ", 
            data.node_addr, hdr.src);

        data.service_token = Some(input.service_token);
        data.service_token.unwrap().touch();

//        data.dest_addr = hdr.dest;
        data.source_req_line = input.source_line_id;
//        data.req_broadcast_instant = Some(Instant::now());

        data.modify_pipe_items = Some( |data, input, alloc| {
 //           let msg = input.msg.unrap();
            let msg = input.msg.as_ref().unwrap();
            let hdr: &SecondLayerHeader = msg.header().into();
            alloc.extend(
                data.lines.as_ref().unwrap().iter()
                .filter(|&line| *line != data.source_req_line.unwrap())
                .map(|&line_id| {                                            
                    NodeCommand(line_id,
                        SecondLayerCommand::SendAsIs(input.msg.as_ref().unwrap().clone(),
 //                           hdr.clone(),
                            data.service_token.clone()
                        )                                            
                ).into()
            }));

            if hdr.dest == NodeAddress::ANY {
                let hdr = SecondLayerHeader::new( 
                    BlockType::AddressMapResponse,
                    data.node_addr,
                    hdr.dest,
                );
                alloc.push_done(          
                    NodeCommand(data.source_req_line.unwrap(), 
                        SecondLayerCommand::Send(input.msg.as_ref().unwrap().clone(), hdr, data.service_token)).into()
                );   
            }
            alloc.take_current();
        });
        true
    }
}

struct OnDestinationRespond;
impl TransitionHandle for OnDestinationRespond {
	type DataType = FSMStateData;
	type SignalType = FSMSignals;

	fn call(&self, input: &<Self::SignalType as SignalInput>::Item, 
        data: &mut Self::DataType) -> bool {
        // prevent duplicate responses
        if data.service_token == Some(input.service_token) {
            return false;
        }
        data.service_token = Some(input.service_token);
        data.modify_pipe_items = Some( |data, input, alloc| {
            let msg = input.msg.as_ref().unwrap();
            let hdr: &SecondLayerHeader = msg.header().into();

            let hdr = SecondLayerHeader::new( 
                BlockType::AddressMapResponse,
                data.node_addr,
                hdr.src,
            );
            let msg = Chunk::from_message(
                MessageBodyType::TimeStamp(Instant::now()));

            log::info!("OnDestinationRespond, {} sending to {} on line {}", data.node_addr, hdr.dest, input.source_line_id.unwrap());
    
            alloc.push_done(
                NodeCommand(input.source_line_id.unwrap(), 
                    SecondLayerCommand::Send(msg, 
                        hdr, 
                        Some(input.service_token)))  
                    .into());
            alloc.take_current();
        });   
        true
    }
}

struct OnResponseReceived;
impl OnResponseReceived {
    fn update_map_on_resp(&self, src: NodeAddress, 
        input: &<<Self as TransitionHandle>::SignalType as SignalInput>::Item,
        data: &mut <Self as TransitionHandle>::DataType) {
        
        let delay =
            match input.msg.as_ref().unwrap().body() {
                MessageBodyType::TimeStamp(ts) => ts.elapsed(),
                _ => unreachable!(),
        };

        let new_entry =  AddressMapEntry { 
            line_id: input.source_line_id.unwrap(), 
            oneway_delay: delay
        };
        
        data.address_map.as_ref().unwrap().write().unwrap()
            .entry(src)
            .and_modify(|entry| {
                if entry.oneway_delay > delay { *entry = new_entry }
            })
            .or_insert(new_entry);
    }
}

impl TransitionHandle for OnResponseReceived {
	type DataType = FSMStateData;
	type SignalType = FSMSignals;

	fn call(&self, input: &<Self::SignalType as SignalInput>::Item, 
        data: &mut Self::DataType) -> bool {

        let msg = input.msg.as_ref().unwrap();
        let hdr: &SecondLayerHeader = msg.header().into();

        self.update_map_on_resp(hdr.src, input, data);

        log::info!("{} OnResponseReceived, before token check", data.node_addr);
        if data.service_token != Some(input.service_token) {
            return false;
        }

//        data.source_req_line = input.source_line_id;

        data.service_token.unwrap().touch();

        data.modify_pipe_items = Some( |data, input, alloc| {
            log::info!("{} OnResponseReceived, resending on line {}", data.node_addr, data.source_req_line.unwrap());

            alloc.take_current();
            alloc.push_done(
                NodeCommand(data.source_req_line.unwrap(),
                    SecondLayerCommand::SendAsIs(input.msg.as_ref().unwrap().clone(), data.service_token)
                ).into()
            );
        });
        true
    }
}


struct OnSourceResponseReceived;
impl OnSourceResponseReceived {
    fn update_map_on_resp(&self, src: NodeAddress, 
        input: &<<Self as TransitionHandle>::SignalType as SignalInput>::Item,
        data: &mut <Self as TransitionHandle>::DataType) {
        
        let delay =
            match input.msg.as_ref().unwrap().body() {
                MessageBodyType::TimeStamp(ts) => ts.elapsed(),
                _ => unreachable!(),
        };

        let new_entry =  AddressMapEntry { 
            line_id: input.source_line_id.unwrap(), 
            oneway_delay: delay
        };
        
        data.address_map.as_ref().unwrap().write().unwrap()
            .entry(src)
            .and_modify(|entry| {
                if entry.oneway_delay > delay { *entry = new_entry }
            })
            .or_insert(new_entry);
    }
}

impl TransitionHandle for OnSourceResponseReceived {
	type DataType = FSMStateData;
	type SignalType = FSMSignals;

	fn call(&self, input: &<Self::SignalType as SignalInput>::Item, 
        data: &mut Self::DataType) -> bool {

//        if data.service_token != Some(input.service_token) {
//            return false;
//        }
        let msg = input.msg.as_ref().unwrap();
        let hdr: &SecondLayerHeader = msg.header().into();
        log::info!("\t\t\tAddress map at {} got source responded from {}, after token check ", 
            data.node_addr, hdr.src);
        
        self.update_map_on_resp(hdr.src, input, data);

        data.modify_pipe_items = Some( |data, input, alloc| {

            let msg = input.msg.as_ref().unwrap();
            let hdr: &SecondLayerHeader = msg.header().into();

            alloc.take_current();
            alloc.push_done(
                NodeEvent(LineId::default(),
                    SecondLayerEvent::ServiceComplete(
                        data.service_token.unwrap(),
                        ServiceResultType::AddressMapUpdate(hdr.src))
                ).into()
            );
        });
        true
    }
}

struct OnDrop;
impl TransitionHandle for OnDrop {
	type DataType = FSMStateData;
	type SignalType = FSMSignals;

	fn call(&self, input: &<Self::SignalType as SignalInput>::Item, 
        data: &mut Self::DataType) -> bool {
        if data.service_token != Some(input.service_token) {
            return false;
        }
        log::warn!("dropping address map srvc on {}", data.node_addr);
        data.source_req_line.take();
        true
    }
}

struct OnServiceTokenExpired; 
impl TransitionHandle for OnServiceTokenExpired {
	type DataType = FSMStateData;
	type SignalType = FSMSignals;

	fn call(&self, input: &<Self::SignalType as SignalInput>::Item, data: &mut Self::DataType) -> bool {
//        data.service_token.take();
        if data.service_token != Some(input.service_token) {
            return false;
        }
        data.source_req_line.take();
//        data.req_broadcast_instant.take();
        true
    }
}

pub struct AddressMapResolver {
//    state_data: FSMStateData,
    node_addr: NodeAddress,
    fsm: FSM<FSMStates, FSMSignals, FSMStateData>,
}

use FSMStates::*;
use SignalsEnum::*;

impl AddressMapResolver {
    pub fn new(
        node_addr: NodeAddress, 
        lines: Arc<Vec<LineId>>,
        address_map: Arc<RwLock<HashMap<NodeAddress, AddressMapEntry>>>,
    ) -> Self {
        use FSMStates::*;

        let mut fsm = 
				FSM::<FSMStates, FSMSignals, FSMStateData>::from_vec(vec![
					ActiveState,
				]);
		fsm.chain_awaiting_state()
            .chain(SignalsEnum::RequestSignal, ActiveState, Some(Box::new(OnRequest)))
            .chain(SignalsEnum::RequestSignal, ActiveState, Some(Box::new(OnRequest)))
            .chain_to_awaiting_state(SignalsEnum::DropSignal,Some(Box::new(OnDrop)))
            .chain(SignalsEnum::ForeignRequestSignal, 
                ActiveState, Some(Box::new(OnForeignRequest)));
//            .chain_to_awaiting_state(SignalsEnum::ServiceTokenExpiredSignal, 
//                Some(Box::new(OnServiceTokenExpired)));
        fsm.chain_awaiting_state()
            .chain_to_awaiting_state(SignalsEnum::DestRespondSignal, Some(Box::new(OnDestinationRespond)));
        fsm.start_chain(ActiveState)
            .chain(SignalsEnum::ResponseReceivedSignal, ActiveState, Some(Box::new(OnResponseReceived)))
            .chain_to_awaiting_state(SignalsEnum::SourceResponseReceivedSignal, Some(Box::new(OnSourceResponseReceived)));

        let state_data = FSMStateData {
            node_addr,
            dest_addr: NodeAddress::default(),
            service_token: None,
            source_req_line: None,
//            req_broadcast_instant: None,
            lines: Some(lines),
            address_map: Some(Arc::clone(&address_map)),
            modify_pipe_items: None,
        };
        fsm.init(state_data);
        fsm.run();

        Self { 
            node_addr,
            fsm,
        }
    }
    
    fn update_map_passively(&self, source_line_id: LineId, hdr: &SecondLayerHeader) {
        if self.fsm.state_data().node_addr != hdr.src {  // do not register self address
            let delay = core::time::Duration::from_secs(1_000_000);
            let new_entry =  AddressMapEntry { 
                line_id: source_line_id, 
                oneway_delay: delay
            };
            self.fsm.state_data().address_map.as_ref().unwrap().write().unwrap()
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
                            let mut signal;
                            if self.fsm.state_data().service_token.unwrap().expired() {
                                signal = FSMSignals(DropSignal, 
                                    SignalInputData {
                                        service_token: *service_token, 
                                        source_line_id: None,
                                        msg: None,
                                        hdr: None,
                                });
        
                                self.fsm
                                    .signal(&signal)
                                    .modify_pipe_items(signal.get(), shared_alloc);        
                            }

                            if service_token.expired() {
                                signal = FSMSignals(ServiceTokenExpiredSignal, 
                                    SignalInputData {
                                        service_token: *service_token, 
                                        source_line_id: None,
                                        msg: None,
                                        hdr: None,
                                    });                             
                            }
                            else
                            if hdr.dest == self.node_addr {
                                signal = FSMSignals(DestRespondSignal, 
                                            SignalInputData {
                                                service_token: *service_token, 
                                                source_line_id: Some(*source_line_id),
                                                msg: Some(msg.clone()),
                                                hdr: None,
                                            }); 
                            } else {
                                signal = FSMSignals(ForeignRequestSignal, 
                                        SignalInputData {
                                            service_token: *service_token, 
                                            source_line_id: Some(*source_line_id),
                                            msg: Some(msg.clone()),
                                            hdr: None,
                                        });  
                            }                                                        
                            self.fsm
                                .signal(&signal)
                                .modify_pipe_items(signal.get(), shared_alloc);
                        },

                        BlockType::AddressMapResponse => {
//                                log::info!("\t\t\tAddress map at {} got response from {}, ", 
//                                self.node_addr, hdr.src);
                            let signal;
                            if service_token.expired() {
                                signal = FSMSignals(ServiceTokenExpiredSignal, 
                                    SignalInputData {
                                        service_token: *service_token, 
                                        source_line_id: None,
                                        msg: None,
                                        hdr: None,
                                    });                             
                            }
                            else {
                                let input = SignalInputData {
                                    service_token: *service_token, 
                                    source_line_id: Some(*source_line_id),
                                    msg: Some(msg.clone()),
                                    hdr: None,
                                };
                                log::info!("\t\t\tAddress map at {} got response from {}, dest: {}", 
                                   self.node_addr, hdr.src, hdr.dest);
                                if self.node_addr == hdr.dest {
//                                    log::info!("\t\t\tAddress map at {} got response from {}, token.elapsed {}", 
//                                   self.state_data.node_addr, hdr.src, self.state_data.service_token.unwrap().elapsed().as_millis());
                                    // for non broadcast request can finish service
                                    if self.fsm.state_data().dest_addr != NodeAddress::default() {
    //                                    let source_line_id = source_line_id;
                                        signal = FSMSignals(SourceResponseReceivedSignal, input);
                                    } 
                                    else {
                                        signal = FSMSignals(ResponseReceivedSignal, input);
                                    }
                                } 
                                else {
                                    signal = FSMSignals(ResponseReceivedSignal, input);
                                }
                            }
                            self.fsm
                                .signal(&signal)
                                .modify_pipe_items(signal.get(), shared_alloc);
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
//                    NodeCommand(_, 
                    SecondLayerCommand::ServiceRequest(
                        service_token,
                        ServiceRequestType::AddressMap(dest)) => {
                            let hdr = SecondLayerHeader::new( 
                                BlockType::AddressMapRequest,
                                self.node_addr,
                                *dest,
                            );
                    
                            let msg = Chunk::from_message(
                                MessageBodyType::TimeStamp(Instant::now()));

                            let input = SignalInputData {
                                service_token: service_token.clone(), 
                                source_line_id: None,
                                msg: Some(msg),
                                hdr: Some(hdr),
                            };

                            let signal = FSMSignals(RequestSignal, input);
         
                            self.fsm
                                .signal(&signal)
                                .modify_pipe_items(signal.get(), shared_alloc);
                    },

                    SecondLayerCommand::DropService(service_token) => {
                        let signal = FSMSignals(DropSignal, 
                            SignalInputData {
                                service_token: *service_token, 
                                source_line_id: None,
                                msg: None,
                                hdr: None,
                        });

                        self.fsm
                            .signal(&signal)
                            .modify_pipe_items(signal.get(), shared_alloc);
                    },

                    _ => (), 
                };        
            },
            _ => unimplemented!(),
        }
    }
}

