use std::collections::{HashMap, BinaryHeap, HashSet};
use std::cmp::Ord;
use core::time::Duration;
use serde::{Serialize};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use instant::Instant;
use futures::channel::mpsc::{unbounded, UnboundedSender, UnboundedReceiver };
use futures::{pin_mut, select, FutureExt};
use futures::StreamExt;
use strum_macros::Display;

use crate::protocol_stack::physical_layer::{PhysicalLayerEvent, PayloadTimestamped};
use crate::protocol_stack::node::{NodeAddress, Node, LineEventSender};
use crate::protocol_stack::node::socket::{SocketProfile};
use crate::protocol_stack::line::{LineId, LineTransceiverId, LineModel};

#[cfg(target_arch = "wasm32")]
use crate::cy_structs::{CyNode, CyEdge, CurrentTransmitInfo};

pub struct MockWorkEntry(NodeAddress, LineModel, NodeAddress);
impl MockWorkEntry {
    pub fn new(n1: NodeAddress, lm: LineModel, n2: NodeAddress) -> Self {
        
        if n1 < n2 {Self(n1, lm, n2)} else {Self(n2, lm, n1)}
    }
}

impl PartialEq for MockWorkEntry {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.2 == other.2
    }
}

impl Eq for MockWorkEntry {
}

impl std::hash::Hash for MockWorkEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.2.hash(state);
    }
}

struct Edge {
    node_addr1: NodeAddress,
    node_addr2: NodeAddress,
    line_model: LineModel,
}

#[derive(Serialize, Display)]
pub enum MockWorkStatus {
    Idle = 0,
    Running,
    Suspended,
    Shutdown,
}

impl From<usize> for MockWorkStatus {
    fn from(n: usize) -> Self {
        match n {
            0 => Self::Idle,
            1 => Self::Running,
            2 => Self::Suspended,
            3 => Self::Shutdown,
            _ => panic!("Unknown Mock Work status"),
        }
    }
}

pub struct MockWork {
    nodes: HashMap<NodeAddress, Mutex<Node>>,
    edges: HashMap<LineId, Edge>,
    transmit_scheduler: Arc<Mutex<TransmitScheduler>>,
    line_delay_range: (u64, u64),
    transmit_schedule_data: Arc<TransmitScheduleData>,
    status: Arc<AtomicUsize>,
    pub(crate) js_loop_cycle_gap: Arc<AtomicU32>,
}

impl MockWork {
    fn wire_node(
        neighbours: &mut HashMap<NodeAddress, Vec<SocketProfile>>, 
        node_addr: NodeAddress,
		socket_profile: SocketProfile,
    )  {	
        neighbours
            .entry(node_addr)
            .or_default()
            .push(socket_profile);
	}

    pub fn weave(wired_nodes: HashSet<MockWorkEntry>, js_loop_cycle_gap: u32) -> Self {
        let mut nodes = HashMap::new();
        let mut edges = HashMap::new();
        let mut neighbours: HashMap<NodeAddress, Vec<SocketProfile>> = HashMap::new();
        let (mut min_line_delay, mut max_line_delay): (u64, u64) = (u64::MAX, 0);
        let js_loop_cycle_gap = Arc::new(AtomicU32::new(js_loop_cycle_gap));

        let (notify_transmit_tx, notify_transmit_rx): 
            (UnboundedSender<ScheduledTransmit>, UnboundedReceiver<ScheduledTransmit>) = unbounded();

        for item in wired_nodes {
            let addr1 = item.0;
            let addr2 = item.2;
  
            let line_model = item.1;
            let line_id = LineId::new();

            if (line_model.propagation_delay.as_millis() as u64) < min_line_delay {
                min_line_delay = line_model.propagation_delay.as_millis() as u64;
            }
            if (line_model.propagation_delay.as_millis() as u64) > max_line_delay {
                max_line_delay = line_model.propagation_delay.as_millis() as u64;
            }

            let socket_profile1 = SocketProfile::new(
                LineTransceiverId::First(line_id),
                line_model.clone(),
                10000,
            );

            let socket_profile2 = SocketProfile::new( 
                LineTransceiverId::Second(line_id),
                line_model.clone(),
                10000,
            );

            Self::wire_node(&mut neighbours, addr1, socket_profile1);
            Self::wire_node(&mut neighbours, addr2, socket_profile2);

            let edge = Edge { node_addr1: addr1, node_addr2: addr2, line_model };
            
            edges.insert(line_id, edge);
        }
        for (node_addr, socket_profiles) in neighbours {
            let node = Node::new(
                node_addr,
                socket_profiles, 
                notify_transmit_tx.clone(),
                Arc::clone(&js_loop_cycle_gap),
            );
            nodes.insert(node_addr, Mutex::new(node));
        }

        let transmit_scheduler = TransmitScheduler::new();

        let transmit_schedule_data = Arc::new(
            TransmitScheduleData {
                recently_sent: AtomicUsize::new(0),
                overall_sent: AtomicUsize::new(0),
                remaining: AtomicUsize::new(0),
                per_line: Mutex::new(HashMap::<LineId, AtomicUsize>::new()),
            });
        let transmit_scheduler = Arc::new(Mutex::new(transmit_scheduler));  
        let status = Arc::new(AtomicUsize::new(MockWorkStatus::Idle as usize));

        crate::target_dependant::spawn(
            transmit_scheduler_loop(
                Arc::clone(&transmit_scheduler),
                notify_transmit_rx,
                Arc::clone(&status),
                Arc::clone(&transmit_schedule_data),
            )
        );

        Self { 
            nodes, 
            edges,
            transmit_scheduler,
            js_loop_cycle_gap,
            line_delay_range: (min_line_delay, max_line_delay),
            transmit_schedule_data,
            status,
        }
    }

    pub async fn run(&mut self) {
        log::debug!("run, status {}", self.status.load(Ordering::Relaxed));

        match self.status.load(Ordering::Relaxed).into() {
            MockWorkStatus::Idle => {
                self.resume_transmits().await;
                self.status.store(MockWorkStatus::Running as usize, Ordering::Relaxed);
            },  
            _ => (),
        }
    }

    fn get_other_side_node(&self, n1: &NodeAddress, lid: &LineId) -> Option<&Mutex<Node>> {
        if let Some(edge) = self.edges.get(lid) {
            return if edge.node_addr1 == *n1 {
                self.nodes.get(&edge.node_addr2)
            } else {
                self.nodes.get(&edge.node_addr1)
            };
        }
        None
    }


 /*   pub fn disconnect_line(&mut self, line_id: LineId) {
        if let Some(edge) = self.edges.get(&line_id) {
            
            let (mut n1, mut n2) = ( 
                self.nodes.get(&edge.node_addr1).unwrap().lock().unwrap(), 
                self.nodes.get(&edge.node_addr2).unwrap().lock().unwrap() );
            
//            let socket1_event_tx = n1.socket_event_tx.clone();    
//            let socket2_event_tx = n2.socket_event_tx.clone();  
            n1.disconnect_line(&line_id); 

            n2.disconnect_line(&line_id);
/*
            self.transmit_scheduler.as_mut().unwrap().unregister_line(
                LineTransceiverId::First(line_id));
            self.transmit_scheduler.as_mut().unwrap().unregister_line(
                    LineTransceiverId::Second(line_id));
  */                  
        }
    }*/
    pub async fn toggle_node_connection(&self, node_addr: &NodeAddress) {
        let mut connected = false;

        log::debug!("toggle_node_connection node {}", node_addr);
        if let Some(node) = self.nodes.get(&node_addr) {
//            connected = node.lock().unwrap().is_connected();
            if let Ok(locked) = node.try_lock() {
                connected = locked.is_connected()
            } else { return; }
        }
        if !connected {
            self.connect_node(node_addr).await;
        } else {
            if let Some(node) = self.nodes.get(&node_addr) {
                log::debug!("disconnecting node {}", node_addr);
                if let Ok(mut locked) = node.try_lock() {
                    locked.disconnect().await;
                }
            }
        }
    }

    pub async fn connect_node(&self, node_addr: &NodeAddress) {
        if let Some(node) = self.nodes.get(&node_addr) {
            if let Ok(mut node) = node.try_lock() {
                node.connect().await;

                if let Ok(mut transmit_scheduler_locked)
                    = self.transmit_scheduler.try_lock() {
                    if let Some(line_event_sender) = node.line_event_sender() {
            
                        for sp in node.socket_profiles() {
            //                    for sp in node_locked.socket_profiles() {
                                let line_id = *sp.id.as_val_ref();
                            if let Some(other) 
                                = self.get_other_side_node(&node.addr(), &line_id) {
            
                                if let Ok(other_locked) = other.try_lock() {
                                    if let Some(other_line_event_sender) = other_locked.line_event_sender() {
                                        let other_id = match sp.id {
                                            LineTransceiverId::First(id) => LineTransceiverId::Second(id),
                                            LineTransceiverId::Second(id) => LineTransceiverId::First(id),
                                        };
                       
                                        transmit_scheduler_locked.register_line(
                                            sp.id,
                                            other_line_event_sender.clone(),
                                        );
            
                                        transmit_scheduler_locked.register_line(
                                            other_id,
                                            line_event_sender.clone(),
                                        );
                                    }
                                } 
                            }       
                        }
                    }
                }
            }
        }
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Mutex<Node>> {
        self.nodes.values()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    
    pub fn node(&self, addr: &NodeAddress) -> Option<&Mutex<Node>> {
        self.nodes.get(&addr)
    }

    pub fn node_addresses(&self) -> impl Iterator<Item = &NodeAddress> {
        self.nodes.keys()
    }

    pub fn lines(&self) -> impl Iterator<Item = &LineId> {
        self.edges.keys()
    }

    pub fn line_delay_range(&self) -> (u64, u64) {
        self.line_delay_range
    }

    pub fn status(&self) -> MockWorkStatus {
        self.status.load(Ordering::Relaxed).into()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn current_transmit_info(&self) -> CurrentTransmitInfo {
        let traffic_samples = self.transmit_schedule_data.per_line
            .lock().unwrap()
            .iter()
            .map(|(lid, transmitted)| (*lid, (transmitted.load(Ordering::Relaxed), 0usize)))
            .into_iter()
            .collect();

        CurrentTransmitInfo {
            being_transmitted: self.transmit_schedule_data.recently_sent.load(Ordering::Relaxed),
            overall_transmitted: self.transmit_schedule_data.overall_sent.load(Ordering::Relaxed),           
            remaining_scheduled: self.transmit_schedule_data.remaining.load(Ordering::Relaxed),
            traffic_samples
        }
    }

    pub fn peer(&self, line_id: &LineId) -> Option<(&Mutex<Node>, &Mutex<Node>)> {
        if let Some(edge) = self.edges.get(line_id) {
            Some( (self.nodes.get(&edge.node_addr1).as_ref().unwrap(), 
                    self.nodes.get(&edge.node_addr2).as_ref().unwrap()) )
        }
        else { None }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn to_json(&self) -> String {
        let cy_nodes = self.nodes()
            .filter_map(|node| {
                if let Ok(node_locked) = node.try_lock() {
                    Some( CyNode { 
                        id: node_locked.addr(), 
                        connected: node_locked.is_connected() 
                    })
                } else { None }
            })
            .collect::<Vec<CyNode>>();

        let cy_edges = &self.edges
            .iter()
            .map(|(&id, edge)| CyEdge { 
                id, 
                source: edge.node_addr1, 
                target: edge.node_addr2,
                propagation_delay: edge.line_model.propagation_delay.as_millis() as u64, 
            })
            .collect::<Vec<CyEdge>>();

        let mut s_nodes = serde_json::to_string(&cy_nodes).unwrap();
        s_nodes.pop();
        s_nodes.push(',');
        let mut s_egdes = serde_json::to_string(&cy_edges).unwrap();
        s_egdes.remove(0);
    
        format!("{}{}",
            s_nodes,
            s_egdes,
        )
    }

    pub async fn suspend_transmits(&mut self)  -> MockWorkStatus {
        log::debug!("in suspend_transmits, status {}", MockWorkStatus::from(self.status.load(Ordering::Relaxed)));
        
        match self.status.load(Ordering::Relaxed).into() {
            MockWorkStatus::Running => {

                for node in self.nodes.values() {
                    if let Ok(mut locked) = node.try_lock() {
                        locked.suspend().await;
                    }
                }
/*
                self.transmit_schedule_data.recently_sent.store(0, Ordering::Relaxed);
                self.transmit_schedule_data.remaining.store(0, Ordering::Relaxed);
                for line_id in self.lines() {
                    self.transmit_schedule_data.per_line.lock().unwrap()
                        .insert(*line_id, AtomicUsize::new(0));
                }*/
                
                self.status.store(MockWorkStatus::Suspended as usize, Ordering::Relaxed);
            },
            _ => (),
        }

        let curr_status = self.status.load(Ordering::Relaxed).into();
        log::debug!("suspend_transmits Current mock work status: {}", curr_status);

        curr_status

    }
    pub async fn resume_transmits(&mut self) -> MockWorkStatus {
        let curr_status = self.status.load(Ordering::Relaxed);
        log::debug!("resume_transmits Current mock work status: {}", 
            MockWorkStatus::from(curr_status));

        match self.status.load(Ordering::Relaxed).into() {
            MockWorkStatus::Idle => {
                for node in self.nodes.values() {
                    if let Ok(mut locked) = node.try_lock() {
                        locked.connect().await;
                    }
                }

                for (line_id, edge) in self.edges.iter() {
                    self.register_line_event_sender_by_line(line_id, &edge);
                }
    
                self.status.store(MockWorkStatus::Running as usize, Ordering::Relaxed);
            },

            MockWorkStatus::Suspended => {
                for node in self.nodes.values() {
                    if let Ok(mut locked) = node.try_lock() {
                        locked.resume().await;
                    }
                }

                for (line_id, edge) in self.edges.iter() {
                    self.register_line_event_sender_by_line(line_id, &edge);
                }
    
                self.status.store(MockWorkStatus::Running as usize, Ordering::Relaxed);
            },
            _ => (),
        }
        let curr_status = self.status.load(Ordering::Relaxed).into();
        log::debug!("resume_transmits Current mock work status: {}", curr_status);

        curr_status
    }

    fn register_line_event_sender_by_line(&self, line_id: &LineId, edge: &Edge) {
        log::debug!("before lock {}", edge.node_addr1);
        let n1 = self.nodes.get(&edge.node_addr1).unwrap().lock().unwrap();
        log::debug!("before lock {}", edge.node_addr2);
        let n2 = self.nodes.get(&edge.node_addr2).unwrap().lock().unwrap();

        let endpoint_id1 
            = n1.socket_profiles().iter()
                .find(|sp| *sp.id.as_val_ref() == *line_id).unwrap().id;
        let endpoint_id2 
            = n2.socket_profiles().iter()
            .find(|sp| *sp.id.as_val_ref() == *line_id).unwrap().id;

        log::debug!("before lock schd");
        let mut transmit_scheduler_locked = self.transmit_scheduler.lock().unwrap();

        if n2.line_event_sender().is_none() {
            log::debug!("{:?}", n2);
            return;
        }
        transmit_scheduler_locked.register_line(
            endpoint_id1,
            n2.line_event_sender().unwrap().clone(),
        );

        if n1.line_event_sender().is_none() {
            log::debug!("{:?}", n1);
            return;
        }
        transmit_scheduler_locked.register_line(
            endpoint_id2,
            n1.line_event_sender().unwrap().clone(),
        );
    }


    pub async fn shutdown(&self) {
        for node in self.nodes.values() {
            if let Ok(mut locked) = node.try_lock() {
                locked.shutdown().await;
            }
        }
//        let _res = self.control_cmd_tx.unbounded_send(ControlCommand::Shutdown);
        self.status.store(MockWorkStatus::Shutdown as usize, Ordering::Relaxed);
    }
}

impl Drop for MockWork {
    fn drop(&mut self) {
//        self.shutdown();
        log::debug!("Mock work dropping");
    }
}

pub struct ScheduledTransmit {
    pub id: LineTransceiverId,
    pub when: Instant,
    pub what: PayloadTimestamped,
}

impl PartialEq for ScheduledTransmit {
    fn eq(&self, other: &Self) -> bool {
        self.when == other.when
    }
}

impl Eq for ScheduledTransmit {}

impl Ord for ScheduledTransmit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.when.cmp(&other.when)
//        self.when.cmp(&other.when).reverse()
    }
}

impl std::cmp::PartialOrd for ScheduledTransmit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
  //      self.when.partial_cmp(&other.when)
        match self.when.partial_cmp(&other.when) {
            Some(ord) => Some(ord.reverse()),
            None => None
        }
    }
}
/*
enum ControlCommand {
    CancelCurrentTransmits,
    ResumeCurrentTransmits,
    Shutdown,
}
*/
struct TransmitScheduleData {
    recently_sent: AtomicUsize,
    overall_sent: AtomicUsize,
    remaining: AtomicUsize,
    per_line: Mutex<HashMap<LineId, AtomicUsize>>,
}

pub struct TransmitScheduler {
    schedule: BinaryHeap<ScheduledTransmit>,
    line_senders: HashMap<LineTransceiverId, LineEventSender>,
}

impl TransmitScheduler {
    fn new() -> Self {

        Self {
            schedule: BinaryHeap::new(),
            line_senders: HashMap::new(),
        }
    }

    fn register_line(&mut self, 
        id: LineTransceiverId,
        other_line_event_tx: LineEventSender, 
    ) {
        if let Some(tx) = self.line_senders.get_mut(&id) {
            *tx = other_line_event_tx;
        } else {
            self.line_senders.insert(id, other_line_event_tx);
        }
    }

    pub fn push(&mut self, scheduled: ScheduledTransmit) {
        self.schedule.push(scheduled);
    }

    fn tick(&mut self, status: &Arc<TransmitScheduleData>) -> Option<core::time::Duration> {
        let now = Instant::now();
        let now_shifted = now.checked_add(
            core::time::Duration::from_millis(
                crate::target_dependant::TASK_SWITCH_DELAY_MILLIS/2)).unwrap();
        
        status.recently_sent.store(0, Ordering::Relaxed);
        status.remaining.store(0, Ordering::Relaxed);

        while let Some(scheduled) = self.schedule.peek() {
            if scheduled.when < now_shifted {
                let item = self.schedule.pop().unwrap(); 
                let id = item.id;
                if let Some(tx) = self.line_senders.get(&id) {
//                    let received_after = item.when.duration_since(item.what.transmit_time);

//                    log::info!("{}@{} rx {:}", id, 
//                                received_after.as_millis(),
//                                item.what.data);
                    let line_id = *id.as_val_ref(); 
                    match tx.unbounded_send(
                        (line_id, PhysicalLayerEvent::Received(item.what))) {
                        Ok(_) => {
                            status.recently_sent.fetch_add(1, Ordering::Relaxed);
                            status.overall_sent.fetch_add(1, Ordering::Relaxed);
                            status.remaining.store(self.schedule.len(), Ordering::Relaxed);
                            status.per_line.lock().unwrap().entry(line_id)
                                .and_modify(|sent_per_line|
                                    { sent_per_line.fetch_add(1, Ordering::Relaxed); })
                                .or_insert(AtomicUsize::new(0));
                        },
                        Err(_err) => (), //panic!("{}", _err),
                    } 
                }
            } else { 
                // return delay after which the top item to be transmitted
                status.remaining.store(self.schedule.len(), Ordering::Relaxed);
                return Some(
                    scheduled.when.duration_since(now_shifted)
                ); 
            }
        }
        None
    }
}

async fn transmit_scheduler_loop(
    scheduler: Arc<Mutex<TransmitScheduler>>,
    mut notify_transmit_rx: UnboundedReceiver<ScheduledTransmit>,
    mw_status: Arc<AtomicUsize>,
    transmit_schedule_data: Arc<TransmitScheduleData>
) {
    let mut running_iter = 0;
    let mut next_wakeup_delay; 
    loop {

        next_wakeup_delay = Duration::from_secs(1_000_000);
        match mw_status.load(Ordering::Relaxed).into() {
            MockWorkStatus::Shutdown => {
                break;
            },

            MockWorkStatus::Running => {
//                next_wakeup_delay = Duration::from_secs(1_000_000);
                if running_iter > 1000 {
//                            log::info!("-------------------------------------pushed = {}", running_iter);
                    running_iter = 0;
                    crate::target_dependant::run_on_next_js_tick().await;
                }

                let next_delay = scheduler.lock().unwrap().tick(&transmit_schedule_data);
                match next_delay  {
                    Some(delay) => { next_wakeup_delay = delay; },             
                
                    None => 
                        if transmit_schedule_data.recently_sent.load(Ordering::Relaxed) == 0 {
                            crate::target_dependant::run_on_next_js_tick().await;
                        }
                }
            },
            MockWorkStatus::Idle
            |
            MockWorkStatus::Suspended => {
                next_wakeup_delay = Duration::from_millis(100);
            }
        }

//        let control_cmd_rx_task = control_cmd_rx.next().fuse();
        let notify_transmit_task = notify_transmit_rx.next().fuse();
		let transmit_scheduled_task 
            = crate::target_dependant::delay(next_wakeup_delay).fuse();

//		pin_mut!(notify_transmit_task, transmit_scheduled_task, control_cmd_rx_task);
   		pin_mut!(notify_transmit_task, transmit_scheduled_task);
 
        select! {
			scheduled_transmit = notify_transmit_task => 
                match mw_status.load(Ordering::Relaxed).into() {
                    MockWorkStatus::Running => {
                        if let Some(item) = scheduled_transmit {
        //                    mw_status.store(MockWorkStatus::Running as usize, Ordering::Relaxed);                        
                            scheduler.lock().unwrap().push(item); 
                            running_iter += 1;
                        }
                    },
                    _ => (),
            },

            () = transmit_scheduled_task => {
                running_iter = 0;
            },
        }
    }
    log::debug!(" +++++++++++ Transmit Sch loop over, status: {}", mw_status.load(Ordering::Relaxed));
} 
