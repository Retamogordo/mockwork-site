//use super::super::first_layer::{LineStatistics};
use super::super::second_layer::{
    LineChannelResult, SecondLayerCommand, 
    ServiceRequestType, ServiceResultType, 
//    LineStatsResult
};
use super::super::{ServiceToken, ChannelType};
use super::super::line::{LineId};
use super::{
    StreamHandle, ChannelError, 
    AcquireStreamHandle};
use crate::protocol_stack::node::{Node, NodeAddress, NodeCommand, service_stream};
use core::time::Duration;

pub trait NodeService {
    type Result;

    fn request_type(&self) -> ServiceRequestType;
//    async fn run(&self) -> Result<T, TopLevelError>;
}
/*
pub struct LineStatsService {
	acq_stream_handle: AcquireStreamHandle,
    batch: usize,
}

impl LineStatsService {
	pub fn new(
        acq_stream_handle: AcquireStreamHandle, 
        batch: usize) -> Self {
 
		Self {
			acq_stream_handle, 
            batch,
		}
	}

	pub async fn run_once(self) -> Option<LineStatistics> {
        let mut stream = self.acq_stream_handle.acquire().await
            .map_err(|_| unimplemented!() )?;

        match stream.event().await {
            Some(event) => match &event {
                ServiceResultType::LineStats(res) => {
                    match res {
                        LineStatsResult::Ready(stats) => Some(*stats),
                        _ => None,
                    }
                }
                _ => unreachable!(),
            },
            None => None,
        }	                     
	}
}

impl NodeService for LineStatsService {
    type Result = LineStatistics;

    fn request_type(&self) -> ServiceRequestType {
        ServiceRequestType::LineStats(self.batch)   
    }
}
*/
pub struct AddressMapService {
    dest: NodeAddress,
    request_result: Result<AcquireStreamHandle, ChannelError>,
}

impl AddressMapService {
	pub fn new(
        src: &mut Node,
		dest: NodeAddress, 
		expires: Duration, 
    ) -> Self {
        Self {
            dest,
            request_result: Self::send_request(src, dest, expires),
		}
	}

    fn send_request(src: &mut Node, dest: NodeAddress, expires: Duration) -> Result<AcquireStreamHandle, ChannelError> {
		if !src.is_connected() {
			return Err(ChannelError(ServiceToken::default(), LineChannelResult::Disconnected));
		}

		let channel_token = ServiceToken::durable(expires, ChannelType::Service);
//		let channel_token = ServiceToken::ageless();

		let node_cmd_tx = src.node_cmd_tx.clone().unwrap();

		let (acq_stream_handle, service_handle) = service_stream( src.addr,
													dest,
													LineId::default(),
													channel_token,
													node_cmd_tx,
												);

		src.service_manager_tx.as_mut().unwrap().unbounded_send(service_handle)
			.map_err(|_err| {
				ChannelError(channel_token, LineChannelResult::Disconnected)
			} )?;
	
		src.command( 
			NodeCommand(LineId::default(), 
			SecondLayerCommand::ServiceRequest(
				channel_token,
				ServiceRequestType::AddressMap(dest)
			))
		)
		.map_err(|_err| ChannelError(channel_token, LineChannelResult::Disconnected))?;

		Ok(acq_stream_handle)
    }

	pub async fn run_once(self) -> Option<NodeAddress> {
        if let Ok(acq_stream_handle) = self.request_result {
            if let Ok(mut stream) = acq_stream_handle.acquire().await {
 //               .map_err(|_| unimplemented!() )?;

                match stream.event().await {
                    Some(event) => match &event {
                        ServiceResultType::AddressMapUpdate(dest) => Some(*dest),
                        
                        _ => None,
                    },
                    None => None,
                }
            } else {
                log::debug!("Addr map handle not acquired");
                None
            }
    
        } else { None }	                     
	}
}

impl NodeService for AddressMapService {
    type Result = NodeAddress;

    fn request_type(&self) -> ServiceRequestType {
        ServiceRequestType::AddressMap(self.dest)   
    }
}

pub struct PeerChannelService {
    dest: NodeAddress,
    request_result: Result<AcquireStreamHandle, ChannelError>,
}
    
impl PeerChannelService {
    pub fn new(
        src: &mut Node,
        dest: NodeAddress, 
        expires: Option<Duration>, 
    ) -> Self {
        Self {
            dest,
            request_result: Self::send_request(src, dest, expires),
        }
    }

    fn send_request(src: &mut Node, dest: NodeAddress, expires: Option<Duration>) -> Result<AcquireStreamHandle, ChannelError> {
		if !src.is_connected() {
			return Err(ChannelError(ServiceToken::default(), LineChannelResult::Disconnected));
		}
        let channel_token;

        if let Some(expires) = expires {
            channel_token = ServiceToken::durable(expires, ChannelType::Service);
        } else {
            channel_token = ServiceToken::ageless();
        }
//		let channel_token = ServiceToken::durable(expires, ChannelType::Service);
		let line_id;
		if let Some(entry) = src.address_map.read().unwrap().get(&dest) {
			line_id = entry.line_id.clone();
		}
		else {
			log::debug!("Not found target node {} at {}", dest, src.addr);
			return Err(ChannelError(channel_token, LineChannelResult::NodeNotFound));
		}	

		let node_cmd_tx = src.node_cmd_tx.clone();

		let (acq_stream_handle, service_handle) = service_stream( src.addr,
													dest,
													line_id,
													channel_token,
													node_cmd_tx.unwrap(),
												);

		src.service_manager_tx.as_mut().unwrap().unbounded_send(service_handle)
			.map_err(|_err| {
				ChannelError(channel_token, LineChannelResult::Disconnected)
			} )?;
	
		src.command( 
			NodeCommand(LineId::default(), 
			SecondLayerCommand::ServiceRequest(
				channel_token,
				ServiceRequestType::PeerChannel(dest)
			))
		)
		.map_err(|_err| {
			ChannelError(channel_token, LineChannelResult::Disconnected)
		})?;

		Ok(acq_stream_handle)
    }

    pub async fn run_once(self) -> Option<StreamHandle> {
        if let Ok(acq_stream_handle) = self.request_result {
            match acq_stream_handle.acquire().await {
                Ok(stream) => return Some(stream),
                Err(ChannelError(_, LineChannelResult::NodeNotFound)) 
                    => log::debug!("node not found"),
                Err(err) 
                    => log::debug!("Peer channel handle not acquired {}", err),
            }

        }
        None
	}

}
    
impl NodeService for PeerChannelService {
    type Result = StreamHandle;

    fn request_type(&self) -> ServiceRequestType {
        ServiceRequestType::PeerChannel(self.dest)   
    }
}
