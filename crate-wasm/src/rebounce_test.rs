use core::time::Duration;
use futures;

#[cfg(not(target_arch = "wasm32"))]
use async_std::task:: {block_on};
#[cfg(not(target_arch = "wasm32"))]
use colored::*;
use std::sync::{Arc, Mutex, RwLock};
use crate::protocol_stack::node::node_services::{AddressMapService, PeerChannelService};

use crate::protocol_stack::node::{NodeAddress};
use crate::protocol_stack::node::stream::*;
use crate::protocol_stack::{MessageBodyType};
use crate::mock_work::{MockWork};
use crate::cy_structs::{CyMockWorkState};

#[cfg(not(target_arch = "wasm32"))]
type TargetDependantTestResult = bool;

fn output_box(text: &str) {
    #[cfg(target_arch = "wasm32")]
    crate::output_box(text);
}

pub fn peer_channel_or_search(
        mock_work: &'static RwLock<Option<MockWork>>,
        shared_state: Arc<Mutex<CyMockWorkState>>,    
        src: NodeAddress,
        dest: NodeAddress,
        message: String,
        streams_exp_delay: Option<u64>,
        map_req_exp_delay: u64,
        max_iterations: Option<usize>,
    ) -> crate::target_dependant::CancellableRun {
        if let Ok(mut shared_state_locked) = shared_state.try_lock() {
            shared_state_locked.sessions_running += 1;
        } 
//        log::info!("in peer_channel_or_search");
        let shared_state_cloned = Arc::clone(&shared_state);
        
        crate::target_dependant::spawn_cancellable( 
            async move {
                peer_channel_or_search_task(
                    mock_work,
                    shared_state_cloned,
                    src,
                    dest,
                    message, 
                    streams_exp_delay,
                    map_req_exp_delay,
                    max_iterations,
                ).await;
            },
            move || {
                if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                    shared_state_locked.sessions_running -= 1;
                }        
            }
        ) 
    }

async fn get_peer_channel_handle(
    mock_work: &MockWork,
    src: NodeAddress,
    dest: NodeAddress,
    streams_exp_delay: Option<u64>,
    _map_req_exp_delay: u64,
) -> Option<StreamHandle> {

    if let Ok(mut src_locked) = mock_work.node(&src).unwrap().try_lock() {
        if let Some(_estimated_path_delay) = src_locked.path_delay(&dest) {
            let peer_service = PeerChannelService::new(&mut *src_locked,
                dest,
                streams_exp_delay.and_then(|exp| Some(Duration::from_millis(exp as u64)) )
            );
            drop(src_locked);
            if let Some(stream) = peer_service.run_once().await {
                return Some(stream);
            }

        } else {
            drop(src_locked);
        }
    }
    if let Ok(mut src_locked) = mock_work.node(&src).unwrap().try_lock() {
        let (_, _exp_delay) = mock_work.line_delay_range(); 
        let map_req_exp_delay = 2000;

        let address_map_service = AddressMapService::new(&mut *src_locked, 
            dest,
            Duration::from_millis(map_req_exp_delay));
        drop(src_locked);

        if let Some(res_dest) =  address_map_service.run_once().await {
            if res_dest != dest { return None; }

            if let Ok(mut src_locked) = mock_work.node(&src).unwrap().try_lock() {
                if let Some(_estimated_path_delay) = src_locked.path_delay(&dest) {

                    let peer_service = PeerChannelService::new(&mut *src_locked,
                        dest,
                        streams_exp_delay
                            .and_then(|exp| Some(Duration::from_millis(exp as u64))) );
                    drop(src_locked);
                    return peer_service.run_once().await;
                }
            }
        }  
    }
    None
}

async fn peer_channel_or_search_task(
    mock_work: &'static RwLock<Option<MockWork>>,
    shared_state: Arc<Mutex<CyMockWorkState>>,    
    src: NodeAddress,
    dest: NodeAddress,
    message: String,
    streams_exp_delay: Option<u64>,
    map_req_exp_delay: u64,
    max_iterations: Option<usize>,
)  {
    if let Ok(mock_work) = mock_work.read() {
        let mock_work = mock_work.as_ref().unwrap();

        if let Some(write_stream) = get_peer_channel_handle(&*mock_work, 
            src, dest, 
            streams_exp_delay, 
            map_req_exp_delay).await {

            output_box(&format!("write stream acquired {}\n", src) );

            let incoming_handle;
            if let Ok(dest_locked) = mock_work.node(&dest).unwrap().try_lock() {

                incoming_handle = dest_locked.incoming(); 
            } else { return; }

            if let Some(read_stream) = incoming_handle.acquire().await {

                if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                    shared_state_locked.active_streams += 1;
                    shared_state_locked.total_streams += 1;
                }
            
                let (write_res, read_res) = futures::future::join(
                    write_loop(write_stream, src, message, max_iterations),
                    read_loop(read_stream),
                )
                .await;

                match write_res {
                    Ok(_) => output_box(&format!("write stream over\n") ),
                    Err(err) => {
//                        log::error!("write loop error: {}", err)
                        output_box(&format!("write stream failure: {}\n", err) );
                    },
                }

                match read_res {
                    Ok(data) => {log::debug!("Received at {}: {}", dest, data);}
                    Err(_err) => (), //{log::error!("read loop error: {}", _err)},
                }
            }
            else {
//               log::error!("{}: incoming stream not acquired", dest);
               output_box(&format!("incoming stream not acquired on {}\n", dest) );
               if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                    shared_state_locked.failed_streams += 1;
                }
            }

        }
        else {
           output_box(&format!("write stream not acquired on {}\n", src) );
           if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                shared_state_locked.failed_streams += 1;
            }
        }
        #[cfg(target_arch = "wasm32")]
        ()  
    }
}

async fn write_loop(mut stream: StreamHandle, 
                    _source: NodeAddress,
                    text: String,
                    max_iterations: Option<usize>,
                 ) -> Result<String, ChannelError> {

    let mut i = 0;
    let running = true;
    while running {

        i += 1;
        if i%1 == 0 {
            crate::target_dependant::run_on_next_js_tick().await;
        }
        if let Some(limit) = max_iterations  {
            if i > limit {
                break;
            }
        }
        for ch in text.chars() {
            stream.write( MessageBodyType::Data(format!("{}", ch)) )?;
        }
    }
    Ok(text)
}

async fn read_loop(mut stream: StreamHandle) 
        -> Result<String, ChannelError> {
    let s = "".to_string();

    let mut i = 1;
    let _prev_tick = instant::Instant::now();
    while let Some(_read_data) = stream.read().await {
        if i%1 == 0 {
            crate::target_dependant::run_on_next_js_tick().await;
        }
        i+=1;
    }
    Ok(s)
}

