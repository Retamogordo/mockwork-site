use std::collections::{HashMap, HashSet};
use serde::{Serialize};
//use serde_json::Result;
use serde::ser::SerializeStructVariant;
use crate::protocol_stack::node::{NodeAddress};
use crate::protocol_stack::line::{LineId};
use crate::mock_work::{MockWork, MockWorkStatus};

#[derive (Eq, Hash)]
pub struct CyEdge {
    pub id: LineId, 
    pub source: NodeAddress, 
    pub target: NodeAddress,
    pub propagation_delay: u64,
}

impl PartialEq for CyEdge {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
} 

impl Serialize for CyEdge {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct_variant("", 0, "data",4)?;

        state.serialize_field("id", &self.id)?;
        state.serialize_field("source", &self.source)?;
        state.serialize_field("target", &self.target)?;
        state.serialize_field("propagation_delay", &self.propagation_delay)?;
		state.end()
    }
}

//#[derive(Serialize)]
#[derive (Eq, Hash)]
pub struct CyNode {
    pub id: NodeAddress,
    pub connected: bool,
}
impl PartialEq for CyNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
} 

impl Serialize for CyNode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct_variant("", 0, "data", 2)?;

        state.serialize_field("id", &self.id)?;
        state.serialize_field("connected", &self.connected)?;
		state.end()
    }
}

//pub type CyNodeAddressMap = HashMap<NodeAddress, HashSet<CyNode>>;

pub type CyTrafficSamples = HashMap<LineId, (usize, usize)>;

#[derive(Serialize)]
pub struct CyNodeState {
    pub(crate) connected: bool,
    pub(crate) neighbours: HashSet<NodeAddress>,
}

pub type CyNodeStates = HashMap<NodeAddress, CyNodeState>;

#[derive(Serialize)]
pub struct CyMockWorkState {
//    pub(crate) loop_stopped: bool,
    pub(crate) mw_status: MockWorkStatus,
    pub(crate) active_nodes_map: CyNodeStates,
//    pub(crate) traffic_samples: CyTrafficSamples, 
    pub(crate) active_streams: usize,
    pub(crate) failed_streams: usize,
    pub(crate) total_streams: usize,
    pub(crate) sessions_running: usize,
    pub(crate) running_sessions_limit: usize,
    pub(crate) js_loop_cycle_gap: u32,
    pub(crate) current_transmit_info: CurrentTransmitInfo,
    pub(crate) was_suspended: bool,
}

impl CyMockWorkState {
    pub fn new(mw: &MockWork) -> Self {
        let traffic_samples = 
            mw.lines()
                .map(|&line_id| (line_id, (0, 0)))
                .collect::<CyTrafficSamples>();

        Self {
//            loop_stopped: false,
            mw_status: mw.status(),
            active_nodes_map: CyNodeStates::new(),
//            traffic_samples,
            active_streams: 0,
            failed_streams: 0,
            total_streams: 0,
            sessions_running: 0,
            running_sessions_limit: 20,
            js_loop_cycle_gap: 0,
            was_suspended: false,
            current_transmit_info: CurrentTransmitInfo { 
                being_transmitted: 0,
                overall_transmitted: 0,
                remaining_scheduled: 0,
                traffic_samples 
            }
        }
    }

    pub fn init(&mut self, mw: &MockWork) {
/*        let traffic_samples = 
            mw.lines()
                .map(|&line_id| (line_id, (0, 0)))
                .collect::<CyTrafficSamples>();*/
//        self.loop_stopped = false;
        self.mw_status = mw.status();
        self.active_nodes_map = CyNodeStates::new();
//        self.traffic_samples = traffic_samples;
        self.active_streams = 0;
        self.failed_streams = 0;
        self.total_streams = 0;
        self.sessions_running = 0;
        self.js_loop_cycle_gap = mw.js_loop_cycle_gap.load(std::sync::atomic::Ordering::Relaxed);
        self.was_suspended = false;
    }
} 

#[derive(Serialize)]
pub struct CurrentTransmitInfo { 
    pub being_transmitted: usize, 
    pub overall_transmitted: usize,
    pub remaining_scheduled: usize,
    pub traffic_samples: CyTrafficSamples, 
}
