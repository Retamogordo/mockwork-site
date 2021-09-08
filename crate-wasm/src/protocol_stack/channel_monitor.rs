use std::collections::{HashMap};
use crate::pipe::{EventTarget, SharedPipeAlloc};
use super::first_layer::{FirstLayerCommand, FirstLayerEvent};
use super::line::{LineModel};
use super::{FirstLayerHeader, ChannelToken, *};
use super::{TimerOwnerToken};
use super::node::NodeAddress;
use crate::protocol_stack::layer_pipe_item::{*};

enum ChannelMode {
    Inclusive(usize),
    Exclusive,
}

impl ChannelMode {
    fn max_channels(&self) -> usize {
        match self {
            Self::Inclusive(n) => *n,
            Self::Exclusive => 1,
        }
    }
}

struct RetrialToken {
    timer_token: TimerOwnerToken,
    retrials: u32,
}

pub struct ChannelMonitor {
    node_addr: NodeAddress,
    active_tokens: Vec<ChannelToken>,
    waiting_tokens: HashMap<ChannelToken, RetrialToken>,
    line_model: LineModel,
    mode: ChannelMode,
    max_channels: usize,
}

impl ChannelMonitor {
    const RETRIALS: u32 = 3;

    pub fn new(node_addr: NodeAddress, line_model: LineModel, max_channels: usize) -> Self {
        Self {
            node_addr,
            active_tokens: Vec::new(),
            waiting_tokens: HashMap::new(),
            line_model,
            mode: ChannelMode::Inclusive(max_channels),
            max_channels,
        }
    }

    fn on_received(&mut self, hdr: FirstLayerHeader, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        match hdr.chunk_type {
            ChunkType::ChannelResponse => {
                self.on_response(&hdr.channel_token.unwrap(), shared_alloc);            
            },
            ChunkType::ChannelRequest(_) => {
                self.accept_from_foreign(hdr, shared_alloc);
            },
            ChunkType::ChannelDrop => {
                self.on_drop_request(&hdr.channel_token.unwrap(), shared_alloc);
            }, 
            ChunkType::ChannelKeepAlive => {
                if let Some(token) = self.active_tokens.iter_mut()
                    .find(|t| Some(**t) == hdr.channel_token) {
                        token.touch();
                        shared_alloc.replace_propagate_current(
                            (FirstLayerEvent::ChannelKeptAlive(token.clone())).wrap().into());
                        }
            }, 
            _ => {
                if let Some(token) = self.active_tokens.iter_mut()
                    .find(|t| Some(**t) == hdr.channel_token) {
                        token.touch();
                        let chunk_received_event = shared_alloc.take_current();
                        shared_alloc.push_done(chunk_received_event.unwrap());
                }
            }
        }
    }

    fn on_response(&mut self, channel_token: &ChannelToken, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {

        if let Some(_) = self.waiting_tokens.remove_entry(&channel_token) {
            if !self.active_tokens.contains(&channel_token) {
                let mut channel_token = channel_token.clone();
                if channel_token.touch() {
                    self.active_tokens.push(channel_token);

                    shared_alloc.replace_propagate_current(
                        (FirstLayerEvent::ChannelEstablished(channel_token)).wrap().into());
                } else {
                    shared_alloc.replace_propagate_current(
                        (FirstLayerEvent::ChannelExpired(channel_token)).wrap().into());    
                }
            }
        }
    }

    fn accept_from_foreign(&mut self, 
        header: FirstLayerHeader,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {
        if self.mode.max_channels() > self.total_channels() {
            let mut header = header.clone();
            header.chunk_type = ChunkType::ChannelResponse;
            let token = &header.channel_token.unwrap();

            if !self.active_tokens.contains(&token) {
                let mut token = token.clone();
                if token.touch() {
                    self.active_tokens.push(token);

                    let resp = Chunk::from_message(
                        MessageBody::Data("first layer channel response".to_string()));
                    shared_alloc.push_done(
                        (FirstLayerCommand::Send(resp, 
                                                header)).wrap().into()
                    );

                    shared_alloc.replace_propagate_current(
                        (FirstLayerEvent::IncomingDataChannel(token)).wrap().into());
                }
            }
        } 
	}
  
	fn send_channel_request(&mut self, channel_token: ChannelToken, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
		let request = Chunk::from_message(
            MessageBody::Data("first layer channel request".to_string()));

		let header = FirstLayerHeader::new(
			ChunkType::ChannelRequest(false),
			Some(channel_token));

        let mut timer_token = None;

		if let Some(retrial_token) = self.waiting_tokens.get(&channel_token) {
            if !self.active_tokens.contains(&channel_token) {
                timer_token = Some(retrial_token.timer_token.clone());
            }
        } else {
            let retrial_token = RetrialToken { 
                timer_token: TimerOwnerToken::new(2*self.line_model.probable_delay()),
                retrials: 0,
            };	
            timer_token = Some(retrial_token.timer_token.clone());

            self.waiting_tokens.insert(channel_token, retrial_token);
        }

		if let Some(timer_token) = timer_token {
            shared_alloc.push_done(
                (FirstLayerCommand::StartTimer(timer_token)).wrap().into());

            shared_alloc.push_done(
                (FirstLayerCommand::Send(request, header)).wrap().into());
        }
	}
    fn drop_channel_inner(&mut self, 
        channel_token: ChannelToken, 
        expired: bool,
        shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>
    ) {
        if let Some(ind) = self.active_tokens.iter().position(|t| *t == channel_token) {
            self.active_tokens.swap_remove(ind);

            self.make_drop_request(channel_token, shared_alloc);

            if expired {
                shared_alloc.replace_propagate_current(
                    (FirstLayerEvent::ChannelExpired(channel_token)).wrap().into());
            } else {
                shared_alloc.replace_propagate_current(
                    (FirstLayerEvent::ChannelDropped(channel_token)).wrap().into());
            }
        } else if let Some(_)
            = self.waiting_tokens.remove_entry(&channel_token) {
            self.make_drop_request(channel_token, shared_alloc);
        }
    }

    fn on_drop_request(&mut self, channel_token: &ChannelToken, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        if let Some(ind) = self.active_tokens.iter().position(|t| t == channel_token) {
            self.active_tokens.swap_remove(ind);

            shared_alloc.replace_propagate_current(
                (FirstLayerEvent::ChannelDropped(channel_token.clone())).wrap().into()
            );
        }
    }

    fn make_drop_request(&self, channel_token: ChannelToken, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        let drop_request = Chunk::from_message(
            MessageBody::Data("first layer channel drop".to_string()));

        let header = FirstLayerHeader::new(
            ChunkType::ChannelDrop,
            Some(channel_token));
        shared_alloc.extend(
            (0..Self::RETRIALS)
            .map(|_| (FirstLayerCommand::Send(drop_request.clone(), header)).wrap().into() )
        );
    }

    fn drop_channel(&mut self, channel_token: ChannelToken, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        self.drop_channel_inner(channel_token, false, shared_alloc);
    }

    fn on_line_timeout(&mut self, timer_token: TimerOwnerToken, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        enum RetrialOptions {
            Retry(ChannelToken),
            Giveup(ChannelToken),
        }
        
        let mut token_pairs = self.waiting_tokens.iter_mut();

        token_pairs
            .find(|(_ch_t, rt)| rt.timer_token == timer_token)
            .and_then(|(ch_t, rt)| {
                rt.retrials += 1;
                if rt.retrials > Self::RETRIALS {
                    Some(RetrialOptions::Giveup(*ch_t))
                   
                } else {
                    Some(RetrialOptions::Retry(*ch_t))
                    
                }
            })
            .and_then(|opt| {
                match opt {
                    RetrialOptions::Giveup(ch_t) => {
                        self.waiting_tokens.remove(&ch_t);
                        shared_alloc.replace_propagate_current(
                                (FirstLayerEvent::HandshakeFailure(ch_t)).wrap().into()
                        );
                    },
                    RetrialOptions::Retry(ch_t) => {
                        self.send_channel_request(ch_t, shared_alloc);
                    }
                }
                Some(())
            })
            .or_else(|| {
//                shared_alloc.clear();

                shared_alloc.replace_propagate_current(
                    FirstLayerEvent::TimerFinished(timer_token).wrap().into()
                );	
                None    
            });      		
    }

    fn total_channels(&self) -> usize {
        self.active_tokens.len() + self.waiting_tokens.len()
    }

    fn set_mode(&mut self, exclusive: bool) -> bool {
        if exclusive {
            if (ChannelMode::Exclusive).max_channels() > self.total_channels() {
                self.mode = ChannelMode::Exclusive;
                return true;
            }
        } else if self.max_channels > self.total_channels() {
            self.mode = ChannelMode::Inclusive(self.max_channels);
            return true;
        }
        false
    }

    fn on_too_many_channels(&mut self, channel_token: ChannelToken, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {
        log::debug!("TOO MANY CHANNELS: {} on {}", self.total_channels(), self.node_addr);
//        shared_alloc.clear();
        shared_alloc.push(
            FirstLayerEvent::TooManyChannels(channel_token, self.total_channels()).wrap().into()
        );	
    }
}

impl EventTarget<LayerPipeItem> for ChannelMonitor {
    fn notify(&mut self, shared_alloc: &mut SharedPipeAlloc<LayerPipeItem>) {

        match shared_alloc.current() {
            LayerPipeItem::SomeEvent(_, LayerEvent::FirstLayer(event)) =>
                match event {
                    FirstLayerEvent::ChunkReceived(chunk, _at) => {
                        let hdr: &FirstLayerHeader = chunk.header().into();
                        self.on_received(hdr.clone(), shared_alloc);
                    },
        
                    FirstLayerEvent::TimerFinished(timer_token) => {
                        let timer_token = timer_token.clone();
                        self.on_line_timeout(timer_token, shared_alloc);
                    },
        
                    _ => (),
                }
                LayerPipeItem::SomeCommand(_, LayerCommand::FirstLayer(cmd)) => 

                    match cmd {
                        FirstLayerCommand::ChannelRequest(channel_token, exclusive) => {
                            if self.set_mode(*exclusive) {
                                self.send_channel_request(*channel_token, shared_alloc);
                            } else {
                                self.on_too_many_channels(*channel_token, shared_alloc);
                            }		
                        },
        
                       FirstLayerCommand::CheckExpiredTokens => {
                            self.active_tokens.retain(|token| {
                                if token.expired() {
                                    let drop_request = Chunk::from_message(
                                        MessageBody::Data("first layer channel drop".to_string()));
                            
                                    let header = FirstLayerHeader::new(
                                        ChunkType::ChannelDrop,
                                        Some(*token));
                                    shared_alloc.extend(
                                        (0..Self::RETRIALS)
                                        .map(|_| (FirstLayerCommand::Send(drop_request.clone(), header)).wrap().into() )
                                    );
                                                    
                                    shared_alloc.push(
                                        (FirstLayerEvent::ChannelExpired(*token)).wrap().into());
                                    return false;
                                } else {
     //                               log::info!("{} send keep alive {}", addr, *token);
                                    let keep_alive_request = Chunk::from_message(
                                        MessageBody::Data("first layer channel keep alive".to_string()));
                            
                                    let header = FirstLayerHeader::new(
                                        ChunkType::ChannelKeepAlive,
                                        Some(*token));
                                    shared_alloc.extend(
                                       (0..Self::RETRIALS)
                                        .map(|_| (FirstLayerCommand::Send(keep_alive_request.clone(), header)).wrap().into() )
                                    );
                                    true
                                }
                            });
                            shared_alloc.take_current();
                    
                        },
                        
                    FirstLayerCommand::DropService(channel_token) => {
                        self.drop_channel(*channel_token, shared_alloc);
                    },
                    _ => (),
                },
                _ => unimplemented!(),
        }
    }
}
