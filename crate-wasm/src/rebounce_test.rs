use core::time::Duration;
//use log::{info, warn};
use futures;


//use async_std::task;
#[cfg(not(target_arch = "wasm32"))]
use async_std::task:: {block_on};
#[cfg(not(target_arch = "wasm32"))]
use colored::*;
use std::sync::{Arc, Mutex, RwLock};
use crate::node_services::{AddressMapService, PeerChannelService};
use crate::output_box;

//use crate::protocol_stack::second_layer::{*};
use crate::protocol_stack::second_layer::{LineChannelResult,
    //SecondLayerCommand, 
    //ServiceRequestType, 
    ServiceResultType
};
//use crate::protocol_stack::layer_pipe_item::{ NodeCommand};
//use crate::protocol_stack::{ServiceToken, ChannelType};

//use crate::protocol_stack::line::*;
use crate::protocol_stack::node::{NodeAddress};
use crate::protocol_stack::node::stream::*;
//use crate::protocol_stack::node::node_services::{TrafficWatchService};
use crate::protocol_stack::{MessageBodyType};
use crate::mock_work::{MockWork};
use crate::cy_structs::{CyMockWorkState};

#[cfg(not(target_arch = "wasm32"))]
type TargetDependantTestResult = bool;
#[cfg(target_arch = "wasm32")]
type TargetDependantTestResult = ();

//use futures::future::{Abortable, AbortHandle, Aborted};

pub fn peer_channel_or_search(
    //    mock_work: &'static MockWork,
        mock_work: &'static RwLock<Option<MockWork>>,
        shared_state: Arc<Mutex<CyMockWorkState>>,    
        src: NodeAddress,
        dest: NodeAddress,
        message: String,
        _streams_exp_delay: u64,
        map_req_exp_delay: u64,
        failed_peers: Arc<Mutex<std::collections::HashMap<(NodeAddress, NodeAddress), usize>>>,
    ) -> crate::target_dependant::CancellableRun {
        if let Ok(mut shared_state_locked) = shared_state.try_lock() {
            shared_state_locked.sessions_running += 1;
        } 
//        log::info!("in peer_channel_or_search");
        let shared_state_cloned = Arc::clone(&shared_state);
    
//        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        
        crate::target_dependant::spawn_cancellable( 
            async move {
                peer_channel_or_search_task(
                    mock_work,
                    shared_state_cloned,
                    src,
                    dest,
                    message, 
//                    streams_exp_delay,
                    map_req_exp_delay,
                    failed_peers,
                ).await;
            },
            move || {
                if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                    shared_state_locked.sessions_running -= 1;
//                    log::info!("sessions left: {}", shared_state_locked.sessions_running);
                }         
//                log::warn!("session cancelled");
            }
        ) 
    }
/*
pub fn peer_channel_or_search(
//    mock_work: &'static MockWork,
    mock_work: &'static RwLock<Option<MockWork>>,
    shared_state: Arc<Mutex<CyMockWorkState>>,    
    src: NodeAddress,
    dest: NodeAddress,
    message: String,
    streams_exp_delay: u64,
) -> AbortHandle {
    if let Ok(mut shared_state_locked) = shared_state.try_lock() {
        shared_state_locked.sessions_running += 1;
    } 

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    
    crate::target_dependant::spawn( 
        async move {
            Abortable::new(
                peer_channel_or_search_task(
                    mock_work,
                    shared_state,
                    src,
                    dest,
                    message, 
                    streams_exp_delay
                ),
                abort_registration
            ).await;
        }
    ); 
    abort_handle       
}
*/    
async fn get_peer_channel_handle(
    mock_work: &MockWork,
    shared_state: Arc<Mutex<CyMockWorkState>>,    
    src: NodeAddress,
    dest: NodeAddress,
//    streams_exp_delay: u64,
    map_req_exp_delay: u64,
) -> Option<StreamHandle> {

    if let Ok(mut src_locked) = mock_work.node(&src).unwrap().try_lock() {
        if let Some(_estimated_path_delay) = src_locked.path_delay(&dest) {
    //        let streams_exp_delay = 150 + 3*estimated_path_delay.as_millis();
            let streams_exp_delay = 1500;
            log::info!("estimated_path_delay = {}", streams_exp_delay);
            let peer_service = PeerChannelService::new(&mut *src_locked,
                dest,
                Duration::from_millis(streams_exp_delay as u64) );
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
                    //                        let streams_exp_delay = 150 + 3*estimated_path_delay.as_millis();
                    let streams_exp_delay = 1500;

                    let peer_service = PeerChannelService::new(&mut *src_locked,
                        dest,
                        Duration::from_millis(streams_exp_delay as u64) );
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
//    streams_exp_delay: u64,
    map_req_exp_delay: u64,
    failed_peers: Arc<Mutex<std::collections::HashMap<(NodeAddress, NodeAddress), usize>>>,
)  {
    if let Ok(mock_work) = mock_work.read() {
        let mock_work = mock_work.as_ref().unwrap();

        if let Some(write_stream) = get_peer_channel_handle(&*mock_work, 
            Arc::clone(&shared_state), src, dest, 
//            streams_exp_delay, 
            map_req_exp_delay).await {

//            log::info!("write stream acquired {}", src);
            output_box(&format!("write stream acquired {}\n", src) );

            let incoming_handle;
            if let Ok(dest_locked) = mock_work.node(&dest).unwrap().try_lock() {

//        let dest_locked = mock_work.node(&dest).unwrap().lock().unwrap();
                incoming_handle = dest_locked.incoming(); 
            } else { return; }
//            drop(dest_locked);

            if let Some(read_stream) = incoming_handle.acquire().await {
//                log::info!("read stream acquired {}", src);

                if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                    shared_state_locked.active_streams += 1;
                    shared_state_locked.total_streams += 1;
                }
            
    //           let write_res = write_loop(write_stream, src, message).await;                 
    //           let read_res = read_loop(read_stream).await; 

                let (write_res, read_res) = futures::future::join(
                    write_loop(write_stream, src, message),
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
                    Err(err) => (), //{log::error!("read loop error: {}", err)},
                }

    /*            if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                    shared_state_locked.total_streams -= 1;
                }
    */
            }
            else {
//               log::error!("{}: incoming stream not acquired", dest);
               output_box(&format!("incoming stream not acquired on {}\n", dest) );
               if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                    shared_state_locked.failed_streams += 1;
                }
                if let Ok(mut failed_peers) = failed_peers.try_lock() {
                    (*failed_peers).entry((src, dest))
                        .and_modify(|times| *times += 1)
                        .or_insert(1);
                }
                //return;
            }

        }
        else {
//           log::error!("{}: write stream not acquired", src);
           output_box(&format!("write stream not acquired on {}\n", src) );
           if let Ok(mut shared_state_locked) = shared_state.try_lock() {
                shared_state_locked.failed_streams += 1;
            }
            if let Ok(mut failed_peers) = failed_peers.try_lock() {
                (*failed_peers).entry((src, dest))
                    .and_modify(|times| *times += 1)
                    .or_insert(1);
            }
        }
  /*      if let Ok(mut shared_state_locked) = shared_state.try_lock() {
            shared_state_locked.sessions_running -= 1;
        }  */  
//    #[cfg(not(target_arch = "wasm32"))]
    //    return test_result;
        #[cfg(target_arch = "wasm32")]
        ()  
    }
}

async fn write_loop(mut stream: StreamHandle, 
                    _source: NodeAddress,
                    text: String ) -> Result<String, ChannelError> {

 //   log::info!("Write loop sending: {}", text);
//    #[cfg(target_arch = "wasm32")]
//    crate::on_start_transmission(serde_json::to_string(&source).unwrap());
    
//    let mut prev_tick = instant::Instant::now();
//    for i in 0..1000 {
    let mut i = 0;
    let mut running = true;
    while running {

        i += 1;
        if i%1 == 0 {
            crate::target_dependant::run_on_next_js_tick().await;
        }
        if i%1000 == 0 {
//            log::info!("write loop running, stream: {}", stream.token());
        }
        if i > 1000 {
//            break;
        }
        for ch in text.chars() {
//            stream.write( MessageBodyType::Data(format!("{}", ch)) )?;
            if stream.write( MessageBodyType::Data(format!("{}", ch)) )
                .is_err() {
                    running = false;
                    break;
                }
        }
    }
//    log::info!("Elapsed {} after writing", prev_tick.elapsed().as_millis());
//    log::warn!("Write loop over, {}", stream.token());

    // wait a bit to not to drop the stream right away
//    crate::target_dependant::delay(core::time::Duration::from_millis(0)).await;

    Ok(text)
}

async fn read_loop(mut stream: StreamHandle) 
        -> Result<String, ChannelError> {
    let mut s = "".to_string();

//    return Ok(s);

    let mut i = 1;
    let _prev_tick = instant::Instant::now();
    while let Some(_read_data) = stream.read().await {
        if i%1 == 0 {
//            log::info!("Elapsed between cycles: {}", prev_tick.elapsed().as_millis());
//            prev_tick = instant::Instant::now();
//            stream.write(MessageBodyType::Data(format!("{}", "keep stream alive ack")));
            crate::target_dependant::run_on_next_js_tick().await;
//            log::info!("read cycle -> {}", s);
        }
        if i%100 == 0 {
//            log::info!("read loop running");
        }
        i+=1;
 //       if i >= 100 {break;}
 //       s = std::format!("{}{}", &s, _read_data);

//        if s.len() > 1 {break;}
 //       crate::target_dependant::run_on_next_js_tick().await;
   //      s.push_str(read_data.bod + "\n");

 //       #[cfg(target_arch = "wasm32")]      
 //       crate::output_box::get_output_box(&target.to_string(), &s);
    }
//      log::info!("Elapsed {} after reading {}", prev_tick.elapsed().as_millis(), i);

//    log::info!("Node {}: read loop over -> {}", stream, s);
    Ok(s)
}

