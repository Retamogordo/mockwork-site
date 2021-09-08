//#![feature(result_into_ok_or_err)]
mod protocol_stack;
mod chained_graph;
mod fsm;
mod pipe;
mod target_dependant;
mod utils;
mod mock_work;
mod rebounce_test;

use core::time::Duration;
use log::{info, warn};
use futures;


//use async_std::task;
#[cfg(not(target_arch = "wasm32"))]
use async_std::task:: {block_on};

use rand::random;
use rand::Rng;

use colored::*;
use clap::{Arg, App};

use crate::protocol_stack::second_layer::{*};
use crate::protocol_stack::line::*;
use crate::protocol_stack::node::{Node, NodeAddress, display_addr_map};
use crate::protocol_stack::node::stream::*;
use crate::protocol_stack::{Chunk};
use crate::mock_work::{MockWork};

//type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct MockWorkBuilder;



#[macro_use]
extern crate lazy_static;

//lazy_static!(
//    static ref INSTANCE: MockWork = MockWorkBuilder::test_instance();
//    );

impl MockWorkBuilder {
    fn with_line_delay(line_delay: u64) -> MockWork {
        let lm1 = LineModel::new(Duration::from_millis(line_delay),
//            Duration::from_millis(transmission_latency),
            0.0);

            let mut mw = MockWork::weave( vec![
                (NodeAddress::new(1,1), lm1.clone(), NodeAddress::new(1,2)),
                (NodeAddress::new(1,2), lm1.clone(), NodeAddress::new(1,3)),
                (NodeAddress::new(1,3), lm1.clone(), NodeAddress::new(1,4)),
                (NodeAddress::new(1,4), lm1.clone(), NodeAddress::new(1,5)),
            
            ]);

            mw.connect_all();

            mw
    }
}

//use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;

static INSTANCE: OnceCell<MockWork> = OnceCell::new();
/*
static INSTANCE: Lazy<MockWork> = Lazy::new(|| {
    MockWorkBuilder::test_instance(100)
});
*/
pub fn init(line_delay: u64, log_level: log::LevelFilter) {
    simple_logger::SimpleLogger::new()
        .with_level(log_level)
        .init().unwrap();

    let mw = MockWorkBuilder::with_line_delay(line_delay);
    INSTANCE.set(mw);

}

fn random_line(mw: &MockWork) -> &LineId {
    let vec = mw.lines();
    let len = vec.len();

    let i = rand::thread_rng().gen_range(0..len);
    vec[i]
}

fn line_stats(addr: &NodeAddress, line_id: LineId, batch: usize) {
    let mut n = INSTANCE.get().unwrap().node(addr).unwrap().lock().unwrap();
/*
    warn_on_error!(
    n.command(line_id, 
        SecondLayerCommand::Send(
            Chunk::from_message("shit".to_string()),
            crate::protocol_stack::SecondLayerHeader::new(
                crate::protocol_stack::BlockType::Data),
            None))
    );
    block_on_target!( 
        async_std::task::sleep(Duration::from_millis(1500))
        );
        
            return;
*/
    let line_stats_handle = n.line_stats_handle(line_id, batch);
   
    let line_stats_handle = 
        match line_stats_handle {
            Ok(handle) => handle,
            Err(err) => {
                log::error!("Error: {}", err);
                return;
            }
        };

    let handle = target_dependant::spawn( async move {
//            wasm_bindgen_futures::spawn_local( async move {
        let res = line_stats_handle.run_once().await;
        
        match res {
            Ok(stats) => {
                log::debug!("{}", stats);
            },
            Err(err) => log::debug!("{}", err),
        } 
    });
    block_on_target!(handle.handle());
                
}


fn main() {

	
       let matches = App::new("My Test Program")
       .arg(Arg::with_name("tests")
       //              .short("f")
                     .long("tests")
                     .takes_value(true))
        .arg(Arg::with_name("log_level")
            //              .short("f")
            .long("log_level")
            .takes_value(true))
        .arg(Arg::with_name("streams")
   //              .short("streams")
                 .long("streams")
                 .takes_value(true))
        .arg(Arg::with_name("streams_expire")
     //            .short("streams_expire")
                 .long("streams_expire")
                 .takes_value(true))
      //               .help("A cool file"))
      .arg(Arg::with_name("line_delay")
      //          .short("n")
                .long("line_delay")
                .takes_value(true))
        .arg(Arg::with_name("line_id")
       //          .short("n")
                 .long("line_id")
                 .takes_value(true))
        .get_matches();

    let num_of_tests: u32 = matches.value_of("tests").unwrap_or("1").parse().unwrap();

    let num_of_streams: u32 = matches.value_of("streams").unwrap_or("1").parse().unwrap();    
    let streams_exp_delay: u64 = matches.value_of("streams_expire").unwrap_or("1000").parse().unwrap();    
    let line_delay: u64 = matches.value_of("line_delay").unwrap_or("10").parse().unwrap();    
    let line_id: usize = matches.value_of("line_id").unwrap_or("1").parse().unwrap();   
    let line_id = LineId::from(line_id); 
    let log_level: log::LevelFilter = matches.value_of("log_level").unwrap_or("Info").parse().unwrap();
	
    init(line_delay, log_level);
    
    let mut test_handles = vec![];

    println!("Running test with {} streams, streams expire after {} ms, line_delay {} ms, log level {}...",
        num_of_streams, streams_exp_delay, line_delay, log_level);
    

//    block_on(INSTANCE.get().unwrap().disconnect_line(&LineId::from(1)));
//    INSTANCE.get().unwrap().connect_line(&LineId::from(1));
    
    let src = NodeAddress::new(1, 1);
    let dest = NodeAddress::new(1, 5);
    let mut src_locked = INSTANCE.get().unwrap().node(&src).unwrap().lock().unwrap();
 
    use crate::protocol_stack::{ServiceToken, ChannelType};
    let service_token = ServiceToken::durable(Duration::from_millis(3000), ChannelType::Service);

    use crate::protocol_stack::protocol_layers::{ NodeCommand};

    let _res = src_locked.command(
        NodeCommand(LineId::default(), 
            SecondLayerCommand::ServiceRequest(
                service_token,
                ServiceRequestType::AddressMap)
        )
    );

    std::thread::sleep_ms(700);


    let traffic_watch_handle = match 
        src_locked.traffic_watch_handle(LineId::from(1)) {
        
            Ok(traffic_watch_handle) => Some(traffic_watch_handle),
        Err(err) => {
            log::error!("Traffic Watch Service acquisition failure: {}", err);
            None
//                return handle;
        }
    };

    drop(src_locked);
    for node in INSTANCE.get().unwrap().nodes() {
        let guard = node.lock().unwrap();
        display_addr_map(&guard);
    }
  
//    std::thread::sleep_ms(300);
//    block_on(INSTANCE.get().unwrap().disconnect_line(&LineId::from(1)));
//    INSTANCE.get().unwrap().connect_line(&LineId::from(1));

    let tstamp = std::time::Instant::now();

    for _i in 0..num_of_streams {
        test_handles.push(
 //           rebounce_test::send_on_channel(
//            rebounce_test::test_peer_channel(
            rebounce_test::test_broadcast_channel(
                &INSTANCE.get().unwrap(),
                &src,
                &line_id,
 //               &dest,
 //               "01234".to_string(),
                "01234567890123456789".to_string(),
 //"Когда нас накрывает шламом, мы не видим соседние дома, 15 метров".to_string(),
                streams_exp_delay
            ).handle()
        );
    }

    let results = block_on(
        futures::future::join_all(test_handles)
    );
 //   let count = 0;
    let passed = results.into_iter().fold(0, |mut count, res| {
        count += if res {1} else {0}; 
        count
    } );
    
    println!("{} tests passed out of {}, time elapsed: {} ms", 
        passed, num_of_streams, tstamp.elapsed().as_millis());

    let traffic_watch_res = block_on(traffic_watch_handle.unwrap().run());

    match traffic_watch_res {
        Ok(count) =>
            log::info!("\t\t\tTraffic Watch result: {}", count),
        Err(_) => 
            log::error!("\t\t\tTraffic Watch result: failure"),
    }

    for node in INSTANCE.get().unwrap().nodes() {
        let guard = node.lock().unwrap();
        display_addr_map(&guard);
    }

    INSTANCE.get().unwrap().shutdown();

    std::thread::sleep(Duration::from_millis(500));
    info!("main exiting");
}

