use std::sync::{Arc};
use super::node::{NodeAddress};
use super::{TimerOwnerToken};
use crate::pipe::{EventTarget, SharedPipeAlloc};
use crate::protocol_stack::second_layer::{SecondLayerCommand, SecondLayerEvent};
use super::line::{LineId};
use crate::protocol_stack::layer_pipe_item::{*};

pub struct ExpiredStreamsCleaner {
    node_addr: NodeAddress,
    lines: Arc<Vec<LineId>>,
    timer_token: Option<TimerOwnerToken>,
}

impl ExpiredStreamsCleaner {
    pub fn new(node_addr: NodeAddress, lines: Arc<Vec<LineId>>) -> Self {
        Self { 
            node_addr,
            lines,
            timer_token: None 
        }
    }
}

impl EventTarget<LayerPipeItem> for ExpiredStreamsCleaner {
    fn notify(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {

        match shared_alloc.current() {
            LayerPipeItem::SomeEvent(_, LayerEvent::SecondLayer(ref event)) => 
                match event {
//                    NodeEvent(_, 
                    SecondLayerEvent::TimerFinished(timer_token) => {
                        if Some(timer_token) == self.timer_token.as_ref() {
                            let timer_token = timer_token.clone();
                     //       log::info!("Cleaner ticking: {}", timer_token.created_at().elapsed().as_millis());
        //                    shared_alloc.push( NodeCommand(LineId::default(), 
                            shared_alloc.replace_propagate_current( NodeCommand(LineId::default(), 
                                SecondLayerCommand::StartTimer(timer_token)).into());
                            shared_alloc.extend(
                                self.lines.iter()
                                    .map(|lid| 
                                        NodeCommand(*lid, 
                                            SecondLayerCommand::CheckExpiredTokens).into())
                            );
        
                        } 
                    },
                    _ => (),
                },

                LayerPipeItem::SomeCommand(_, LayerCommand::SecondLayer(ref cmd)) => 
                    match cmd {

                        SecondLayerCommand::StartExpiredStreamsCleaner(interval) => {
                            if self.timer_token.is_none() {           
            //                    log::info!("Cleaner started with interval of {} ms", interval.as_millis());
                                self.timer_token = Some(TimerOwnerToken::new(*interval));
                                shared_alloc.replace_propagate_current(
                                    (NodeCommand(LineId::default(), 
                                        SecondLayerCommand::StartTimer(self.timer_token.as_ref().unwrap().clone()))).into()
                                );
                            } 
                        },
                        SecondLayerCommand::StopExpiredStreamsCleaner => {
                            self.timer_token.take();
                        },
                        _ => (), 
                    },
                       
            _ => { let _ = self.node_addr; unimplemented!(); }
        }
    }
}