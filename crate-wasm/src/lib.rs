//! # Mockwork
//! ## Asynchronous Network Simulator.
//! ### Introduction
//! This program presents a simulator of nodes interconnected by physical media.
//! Nodes run a protocol stack that facilitate exchange of data between nodes.
//! The code runs on WebAssembly which is loaded to a single Web Worker.
//!
//![screenshot](https://github.com/Retamogordo/mockwork/tree/master/docs/Screenshot_mockwork.png)
//!
//! ### Disclaimer
//! This program was written as a part of my effort to learn Rust language.
//! The design of this network is not based on any existing real network implementation.
//! All terms used here like socket, peer, stream, protocol etc etc should be
//! taken in their broad and vague sense.
//! Although somewhat complex, this program does not pursuit any practical utility but eat up your CPU when
//! running :)
//! 
//! ### Brief introduction of functioning
//! 
//! Nodes are connected by duplex lines which are modeled by only a couple of
//! parameters: propagation delay and noise level.
//! 
//! The bandwidth is limited naturally by JS event loop :)
//! 
//! The line delay is simulated by a timer delay.
//! 
//! Each node run its own event loop in which it listens for Events from line
//! and Commands from frontend.
//! 
//! These items - Commands and Events are passed through a duplex pipe where
//! protocols reside.
//! 
//! Protocols are entities that receive, recognize, process relevant commands and event
//! and, possibly, emit futher items until they are consumed on corresponding
//! endpoint of the pipe.
//! 
//! For now protocols are layered into three layers: "Physical", "First" and "Second".
//! Rusts type system favors natural firewalling of pipe items in such a way that
//! each item must be expicitly converted to the adjacent layer item in order
//! to be eligible to propagate.
//! 
//! Each protocol has a header which wraps a message to be transmitted, on the other
//! line endpoint the headers are stripped off as long as the message bubbles up in 
//! the protocol pipe.
//! 
//! At first nodes are unaware of their mates, the only thing they can do 
//! is to send a message online using physical layer transmit.
//! There is a protocol that broadcasts requests for address map building upon
//! receiving acks from other nodes.
//! There is a protocol for channel establishing on a line between two adjacent
//! nodes, and this channel can serve as a joint for a longer higher layer channel
//! between two arbitrary nodes.
//! On a "frontend" level a concept of stream is introduced (does not implement
//! a formal Rusts stream interface). 
//! Each physical line can serve for data transmitting over multiple logical
//! streams.
//! Streams lazily expire so they can be garbage collected after line failure.
//! The protocol stack is somehow scalable, one can implement a new protocol
//! introducing events and commands along with relevant business logic and hooking
//! it to the protocol stack.
//! 
//! ### User Interface
//! is written in plain Javascript and utilizes [Cytoscape](https://cytoscape.org) for network visualization.
//! WebAssemly code runs network simulation event loops in asynchronous fashion
//! on separate single Web Worker.
//! When the network is actively doing something, Web Worker polls its shared state
//! to postmessage it to UI thread and present some indication on network activity.
//!
//! ### Technical Issues
//! Line delay are simulated by a standard (asynchronously awaited) delays which
//! present a small but constant time drift which, by cumulative effect leads to a
//! significant time divergence very quickly. 
//! So there is a scheduling mechanism aiming to solve this issue, which usually
//! works fine but in theory (and in practice) causes some messages to be
//! sent before time.
//! This is a CPU greedy program, after all there are a plenty of event loops
//! on top of JS runtime, and while it is UI responsive, it turns to be tricky to achieve
//! internal responsiveness ( I consider I failed to get to a satisfactory solution).
//! I suspect my protocol implementation is far from being optimal and represents
//! a serious bottleneck for the whole story.
//! 
//! ### Browser compatibility
//! I used Firefox 89.0 on Linux while developing and tested it a bit with
//! Chromium 93
//! 
//! ### Rust Programming Practices
//! My first thing in Rust, most likely not best practiced.
//! 
//! ### Deployment Problems
//! I lack knowledge regarding deployment process, after two days of
//! trying to adopt a foreign very simple travis.yml I abandoned it 
//! for deploying it just brutally.
//! 
//! ### Acknowledgements
//! to Victor Gavrish for his 
//! [rust-wasm-worker-template](https://github.com/VictorGavrish/rust-wasm-worker-template)
//! 
mod protocol_stack;
mod pipe;
mod target_dependant;
mod utils;
pub mod mock_work;
mod rebounce_test;
mod cy_structs;

use crate::protocol_stack::{ServiceToken};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_logger;

#[cfg(target_arch = "wasm32")]
use console_error_panic_hook;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures;

use core::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use async_std::task:: {block_on};
#[cfg(not(target_arch = "wasm32"))]
use colored::*;
use rand::thread_rng;
use rand::Rng;

use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::protocol_stack::second_layer::{*};
use crate::protocol_stack::line::*;
use crate::protocol_stack::node::{NodeAddress};
use crate::mock_work::{MockWork, MockWorkEntry};
use crate::cy_structs::{CyMockWorkState, CyNodeState};
use std::collections::{HashSet, HashMap};
use once_cell::sync::OnceCell;

struct MockWorkBuilder(
    RwLock<Option<MockWork>>,
);

#[macro_use]
extern crate lazy_static;

lazy_static!(
    static ref CY_MOCK_WORK_SHARED_STATE: Arc<Mutex<CyMockWorkState>> 
            = Arc::new(Mutex::new(CyMockWorkState::new(
                (**INSTANCE.get().unwrap()).read().unwrap().as_ref().unwrap()
            )));
    static ref INFINITE_RUNNING: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref ADDRESS_MAP_SERVICE_TOKENS: Mutex<HashMap<NodeAddress, ServiceToken>> 
        = Mutex::new(HashMap::new());
    static ref CANCEL_HANDLES: Mutex<Vec<crate::target_dependant::CancellableRun>> 
        = Mutex::new(Vec::new());
    );

static INSTANCE: OnceCell<MockWorkBuilder> = OnceCell::new();

impl MockWorkBuilder {
/*    fn create_random_instance(&self,
        num_nodes: u32, 
        num_lines: u32, 
        propagation_delay_range_millis: (u64, u64),
        js_loop_cycle_gap: u32 ) -> bool {

        if let Ok(mut guard) = self.0.try_write() {
            (*guard).take(); // drop current instance

            let lm1 = LineModel::new(Duration::from_millis(10),
    //            Duration::from_millis(10),
                0.0);

            let mut mw = MockWork::weave( vec![
                MockWorkEntry::new(NodeAddress::new(1,1), lm1.clone(), NodeAddress::new(1,2)),
                MockWorkEntry::new(NodeAddress::new(1,2), lm1.clone(), NodeAddress::new(1,3)),
                MockWorkEntry::new(NodeAddress::new(1,3), lm1.clone(), NodeAddress::new(1,4)),
                MockWorkEntry::new(NodeAddress::new(1,4), lm1.clone(), NodeAddress::new(1,5)),
                MockWorkEntry::new(NodeAddress::new(2,1), lm1.clone(), NodeAddress::new(2,2)),
                ]
                .into_iter()
                .collect::<HashSet<MockWorkEntry>>(),

                20
            );

//          mw.run();
            log::info!("Mockwork running");
    //        mw.connect_all();

            *guard = Some(mw);
            return true;
        }
        false
    }
*/
    async fn create_random_instance(&self,
        num_nodes: u32, 
        num_lines: u32, 
        propagation_delay_range_millis: (u64, u64),
        js_loop_cycle_gap: u32 ) -> bool {

        if let Ok(mut guard) = self.0.try_write() {
            (*guard).take(); // drop current instance

            let node_addresses 
                = (0..num_nodes)
                    .map(|n| NodeAddress::new( n/255 + 1, n%255 + 1 ))
                    .collect::<Vec<NodeAddress>>();

            let mut mw = MockWork::weave( 
                (0..num_lines)
                .map(|_| {
                    let mut rng = thread_rng();
                    let mut once_addresses: Vec<&NodeAddress> = 
                        node_addresses.iter().collect();
                    
                    let n_ind1: usize = rng.gen_range(0..node_addresses.len());
                    let n_ind2: usize = rng.gen_range(0..node_addresses.len()-1);

                    let n1 = once_addresses.swap_remove(n_ind1);
                    let n2 = once_addresses.swap_remove(n_ind2);

                    let d1 = propagation_delay_range_millis.0;
                    let d2 = propagation_delay_range_millis.1;

                    let lm = LineModel::new(
                        Duration::from_millis(rng.gen_range(d1..d2)),
                        0.0);

                    MockWorkEntry::new(*n1, lm, *n2)
                })
                .collect::<HashSet<MockWorkEntry>>(),
                
                js_loop_cycle_gap
            );

            mw.run().await;

            log::debug!("Mockwork running");
            *guard = Some(mw);
            return true;
        }
        output_box("Mockwork creation failure\n");
        false
    }

    async fn take_instance(&self) {
        if let Ok(guard) = self.0.try_read() {
            if let Some(mw) = (*guard).as_ref() {
                mw.shutdown().await;
            }
        }

        if let Ok(mut guard) = self.0.try_write() {
            (*guard).take();
        }
    }
}

impl core::ops::Deref for MockWorkBuilder {
    type Target = RwLock<Option<MockWork>>;

    fn deref(&self) -> &<Self as core::ops::Deref>::Target {
        &self.0
    }
}
 
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/cy_funcs.js")]
extern "C" {
    fn on_start_transmission(node_addr: String);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/cy_funcs.js")]
extern "C" {
    fn output_box(data: &str);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn init() -> bool {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::new(log::Level::Info));
    match INSTANCE.set(MockWorkBuilder(RwLock::new(None))) {
        Ok(_) => true,
        _ => false,
    }
} 

#[cfg(not(target_arch = "wasm32"))]
pub fn init() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init().unwrap();
}


#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn load_instance(
    num_nodes: u32, 
    num_lines: u32, 
    min_line_delay: u32,
    max_line_delay: u32,
    js_loop_cycle_gap: u32,
) -> String {

    let mwb = INSTANCE.get().unwrap();

    let min_delay: u64 = if min_line_delay < max_line_delay {min_line_delay as u64} else {max_line_delay as u64};
    let max_delay: u64 = if min_line_delay < max_line_delay {max_line_delay as u64} else {min_line_delay as u64};

    let mut mw_json = "".to_string();
    
    if mwb.create_random_instance(num_nodes, 
        num_lines, 
        (min_delay, max_delay), 
        js_loop_cycle_gap
    ).await {
        if let Ok(mw) = (*mwb).read() {
            CY_MOCK_WORK_SHARED_STATE.lock().unwrap().init(mw.as_ref().unwrap());
            mw_json = mw.as_ref().unwrap().to_json()
        }
    }
    mw_json
 }

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn shutdown_instance() {
    INSTANCE.get().unwrap().take_instance().await;
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn peer_channel_test(src_addr_json: String, dest_addr_json: String) {
    let src_addr = NodeAddress::from(src_addr_json);
    let dest_addr = NodeAddress::from(dest_addr_json);

    crate::target_dependant::spawn(
        peer_channel_run_inner(
            Arc::clone(&INFINITE_RUNNING), 
            src_addr,
            dest_addr,
            Arc::clone(&CY_MOCK_WORK_SHARED_STATE),
            None,
            100
        )
    );
} 

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn random_peer_channel_test() {
    
    crate::target_dependant::spawn(
        infinite_run_inner(
            Arc::clone(&INFINITE_RUNNING), 
            Arc::clone(&CY_MOCK_WORK_SHARED_STATE),
            1000,
            100
        )
    );
} 

fn get_peer() -> Option<(NodeAddress, NodeAddress)> {
    let mw = (**INSTANCE.get().unwrap()).try_read();
    if mw.is_err() {
        return None;
    }
    let tmp = mw.unwrap();
    let mw = tmp.as_ref().unwrap();

    let len = mw.node_count();
    if len == 0 { return None; }

    for _trials in 0..4 {
        let ind: usize = thread_rng().gen_range(0..len);
        if let Some(src_addr) = mw.node_addresses().nth(ind) {
            let src_locked = mw.node(&src_addr).unwrap().lock().unwrap();
            if !src_locked.is_connected() {
                continue;
            }
            
            let len = src_locked.nodes_online().len();
            if len > 0 {
                if let Some(dest_addr) = src_locked.nodes_online().keys()
                    .nth(thread_rng().gen_range(0..len)) {
                    if *dest_addr == NodeAddress::default() { 
                        return None;
                    }

                    if !mw.node(&dest_addr).unwrap().lock().unwrap().is_connected() {
                        continue;
                    }
                    return Some((*src_addr, *dest_addr));
                }
            }
        } 
    }
    None
}

async fn cool_down(shared_state: Arc<Mutex<CyMockWorkState>>) {
    if let Ok(mut mw) = (**INSTANCE.get().unwrap()).write() {
        output_box(&format!("network suspending...") );

        mw.as_mut().unwrap().suspend_transmits().await;
        output_box(&format!("done\n") );
        if let Ok(mut shared_state_locked) = shared_state.try_lock() {
            shared_state_locked.was_suspended = true;
        }
        output_box(&format!("resuming transmits...") );
        mw.as_mut().unwrap().resume_transmits().await;
        output_box(&format!("done\n") );
    } 
}

async fn peer_channel_run_inner(
        running: Arc<AtomicBool>,
        src: NodeAddress,
        dest: NodeAddress,
        shared_state: Arc<Mutex<CyMockWorkState>>,  
        streams_exp_delay: Option<u64>,  
        map_req_exp_delay: u64,
        
    ) {
        if let Ok(mut cancel_handles) = CANCEL_HANDLES.try_lock() {
            let cancel_handle = crate::rebounce_test::peer_channel_or_search(
                &**INSTANCE.get().unwrap(),
                Arc::clone(&CY_MOCK_WORK_SHARED_STATE),
                src,
                dest,
                "0123456789".to_string(),
                streams_exp_delay,
                map_req_exp_delay,
                None
            );   
            cancel_handles.push(cancel_handle);
        }
        #[allow(deprecated)] {
            if running.compare_and_swap(false, true, Ordering::Acquire) {
                log::debug!("Can run only once at a time");
                return;
            }    
        }
        while running.load(Ordering::Relaxed) {
            crate::target_dependant::run_on_next_js_tick().await;
    
            if let Ok(shared_state_locked) = shared_state.try_lock() {
                if shared_state_locked.sessions_running == 0 { 
                    break;
                }
            } else { break; }
        }

        if let Ok(mut cancel_handles) = CANCEL_HANDLES.try_lock() {
            while let Some(h) = cancel_handles.pop() {
                h.cancel().await;
            }
        }
            
        cool_down(shared_state).await;

        running.store(false, Ordering::Relaxed);
        
        log::debug!("mockwork running loop is over");    
    }
   

async fn infinite_run_inner(
    running: Arc<AtomicBool>,
    shared_state: Arc<Mutex<CyMockWorkState>>,  
    streams_exp_delay: u64,  
    map_req_exp_delay: u64,
    
) {
    #[allow(deprecated)] {
        if running.compare_and_swap(false, true, Ordering::Acquire) {
            log::debug!("Can run only once at a time");
            return;
        }
    }

    let mut cancel_handles = vec![];
    while running.load(Ordering::Relaxed) {

        crate::target_dependant::run_on_next_js_tick().await;

        if let Ok(shared_state_locked) = shared_state.try_lock() {
            if shared_state_locked.sessions_running >= shared_state_locked.running_sessions_limit { 
                drop(shared_state_locked);
                crate::target_dependant::delay(instant::Duration::from_millis(199)).await;
                continue; 
            }
        } else { 
            break;
        }
        if let Some((src, dest)) = get_peer() {
            let cancel_handle = crate::rebounce_test::peer_channel_or_search(
                &**INSTANCE.get().unwrap(),
                Arc::clone(&CY_MOCK_WORK_SHARED_STATE),
                src,
                dest,
                "0123456789".to_string(),
                Some(streams_exp_delay),
                map_req_exp_delay,
                Some(50),
            );   
            cancel_handles.push(cancel_handle);
            output_box(&format!("peer ({}, {}) established\n", src, dest));
        }
    }

    while let Some(h) = cancel_handles.pop() {
        h.cancel().await;
    }
    
    cool_down(shared_state).await;

    running.store(false, Ordering::Relaxed);
    
    log::debug!("mockwork running loop is over");    
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn stop_infinite_run() {
    (*INFINITE_RUNNING).store(false, Ordering::Relaxed);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_loop_cycle_gap(gap: u32) {
    if let Ok(mw) = (**INSTANCE.get().unwrap()).try_read() {
        let mw = mw.as_ref().unwrap();
        
        mw.js_loop_cycle_gap.store(gap, Ordering::Relaxed);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_max_running_sessions_num(running_sessions_limit: usize) {
    if let Ok(mut state_locked) = CY_MOCK_WORK_SHARED_STATE.try_lock() {
        log::debug!("running_sessions_limit = {}", running_sessions_limit);
        state_locked.running_sessions_limit = running_sessions_limit;
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn poll_mock_work() -> String {
    poll_mock_work_inner();

    let mut state_locked = CY_MOCK_WORK_SHARED_STATE.lock().unwrap();
    let state_json = serde_json::to_string(&*state_locked).unwrap();
    state_locked.active_streams = 0;
    // touch state after signalling once that loop has been over
    state_locked.was_suspended = false;
//    state_locked.loop_stopped = false;
    state_json   
}

fn poll_mock_work_inner() {
    if let Ok(mw) = (**INSTANCE.get().unwrap()).try_read() {
        let mw = mw.as_ref().unwrap();

        let mut state_locked = CY_MOCK_WORK_SHARED_STATE.lock().unwrap();
        
        mw.nodes().for_each(|node| {
            let node_locked = node.lock().unwrap();

            state_locked.active_nodes_map
                .insert(node_locked.addr(), 
                    CyNodeState { 
                        connected: node_locked.is_connected(), 
                        neighbours:
                            node_locked
                                .nodes_online()
                                .keys()
                                .map(|node_addr| *node_addr)
                                .collect()
                        }
                );
            state_locked.js_loop_cycle_gap = mw.js_loop_cycle_gap.load(std::sync::atomic::Ordering::Relaxed);
            state_locked.current_transmit_info = mw.current_transmit_info();
            state_locked.mw_status = mw.status();
        });  
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_node_address_map(n: String) -> String {
    let src = NodeAddress::from(n);
    if let Ok(mw) = (**INSTANCE.get().unwrap()).try_read() {
        let mw = mw.as_ref().unwrap();
        let node_locked = mw.node(&src).unwrap().lock().unwrap();

        let nodes_online = node_locked.nodes_online();

        let nodes_json = serde_json::to_string(&nodes_online).unwrap();
        nodes_json   
    } else { "".to_string() }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn toggle_node_connection(n: String) {
    let addr = NodeAddress::from(n);

    crate::target_dependant::spawn( async move {
        if let Ok(mw) = (**INSTANCE.get().unwrap()).try_read() {
            let mw = mw.as_ref().unwrap();
            mw.toggle_node_connection(&addr).await;

            if let Some(node) = mw.node(&addr) {
                output_box(&format!("node {} status: {}\n", addr, 
                    node.lock().unwrap().status()));
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn node_refresh_address_map(n: String, map_request_exp_delay: u64) {
    let src = NodeAddress::from(n);
    let dest = NodeAddress::ANY;

    crate::target_dependant::spawn( async move {
        refresh_address_map_inner(&src, &dest, map_request_exp_delay)
        .await;
    });
}

async fn refresh_address_map_inner(
    src: &NodeAddress, 
    dest: &NodeAddress,  
    map_request_exp_delay: u64
) {
    if let Ok(mw) = (**INSTANCE.get().unwrap()).try_read() {
        let mw = mw.as_ref().unwrap();
        let mut src_locked = mw.node(src).unwrap().lock().unwrap();
    
        use crate::protocol_stack::{ChannelType};
        let service_token = ServiceToken::durable(
            Duration::from_millis(map_request_exp_delay), 
            ChannelType::Service);

        use crate::protocol_stack::layer_pipe_item::{ NodeCommand};
        let mut guard = ADDRESS_MAP_SERVICE_TOKENS.lock().unwrap();
        guard.entry(*src).and_modify(|token| *token = service_token).or_insert(service_token);

        let _res = src_locked.command(
            NodeCommand(LineId::default(), 
                SecondLayerCommand::ServiceRequest(
                    service_token,
                    ServiceRequestType::AddressMap(*dest))
            )
        ); 
    }   
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn network_refresh_address_map(seeds: usize, map_request_exp_delay: u64) {
    crate::target_dependant::spawn( async move {

        let map_request_exp_delay = map_request_exp_delay;
        if let Ok(mw) = (**INSTANCE.get().unwrap()).try_read() {
            let mw = mw.as_ref().unwrap();

            let len = mw.node_count();
            if len == 0 { return; }
            for i in 0..core::cmp::min(seeds, len) {
                if let Some(addr) = mw.node_addresses().nth(i) {
                    output_box(&format!("address map seed {}\n", addr) );

                    refresh_address_map_inner(&addr, &NodeAddress::ANY, map_request_exp_delay)
                        .await;
                }
            }
        }   
    });
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn stop_refreshing_address_map() {
    if let Ok(mw) = (**INSTANCE.get().unwrap()).try_read() {
        let mw = mw.as_ref().unwrap();

        for addr in mw.node_addresses() {
            let mut node_locked = mw.node(addr).unwrap().lock().unwrap();
        
            use crate::protocol_stack::layer_pipe_item::{ NodeCommand};
            if let Some(service_token) = ADDRESS_MAP_SERVICE_TOKENS.lock().unwrap().remove(addr) {
                let _res = node_locked.command(
                    NodeCommand(LineId::default(), 
                        SecondLayerCommand::DropService(service_token)
                    )); 
            }
        }
    }   
}
