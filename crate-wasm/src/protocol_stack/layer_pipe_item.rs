use crate::protocol_stack::physical_layer::{PhysicalLayerCommand, PhysicalLayerEvent};
use crate::protocol_stack::first_layer::{FirstLayerCommand, FirstLayerEvent};
use crate::protocol_stack::second_layer::{SecondLayerCommand, SecondLayerEvent};
use crate::protocol_stack::line::LineId;

#[derive(Clone)]
pub enum LayerCommand {
    PhysicalLayer(PhysicalLayerCommand),
    FirstLayer(FirstLayerCommand),
    SecondLayer(SecondLayerCommand),
    None,
}

use LayerCommand::*;

impl std::fmt::Display for LayerCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "{}", "LayerCommand::None"),
            PhysicalLayer(cmd) => write!(f, "{}", cmd),
            FirstLayer(cmd) => write!(f, "{}", cmd),
            SecondLayer(cmd) => write!(f, "{}", cmd),
        }
	}
}

impl From<PhysicalLayerCommand> for LayerCommand {
	fn from(cmd: PhysicalLayerCommand) -> Self {
		Self::PhysicalLayer(cmd)
	}
}
impl From<FirstLayerCommand> for LayerCommand {
	fn from(cmd: FirstLayerCommand) -> Self {
		Self::FirstLayer(cmd)
	}
}
impl From<SecondLayerCommand> for LayerCommand {
	fn from(cmd: SecondLayerCommand) -> Self {
		Self::SecondLayer(cmd)
	}
}

impl From<LayerCommand> for PhysicalLayerCommand {
	fn from(cmd: LayerCommand) -> Self {
		match cmd {
            PhysicalLayer(cmd) => cmd,
            _ => PhysicalLayerCommand::None,
        }
	}
}
impl From<LayerCommand> for FirstLayerCommand {
	fn from(cmd: LayerCommand) -> Self {
		match cmd {
            FirstLayer(cmd) => cmd,
            _ => FirstLayerCommand::None,
        }
	}
}
impl From<LayerCommand> for SecondLayerCommand {
	fn from(cmd: LayerCommand) -> Self {
		match cmd {
            SecondLayer(cmd) => cmd,
            _ => SecondLayerCommand::None,
        }
	}
}

// ------------------------------------------------------------------------------
#[derive(Clone)]
pub enum LayerEvent {
    PhysicalLayer(PhysicalLayerEvent),
    FirstLayer(FirstLayerEvent),
    SecondLayer(SecondLayerEvent),
    None,
}

impl std::fmt::Display for LayerEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "{}", "LayerEvent::None"),
            LayerEvent::PhysicalLayer(ev) => write!(f, "{}", ev),
            LayerEvent::FirstLayer(ev) => write!(f, "{}", ev),
            LayerEvent::SecondLayer(ev) => write!(f, "{}", ev),
        }
	}
}

impl From<PhysicalLayerEvent> for LayerEvent {
	fn from(ev: PhysicalLayerEvent) -> Self {
		Self::PhysicalLayer(ev)
	}
}
impl From<FirstLayerEvent> for LayerEvent {
	fn from(ev: FirstLayerEvent) -> Self {
		Self::FirstLayer(ev)
	}
}
impl From<SecondLayerEvent> for LayerEvent {
	fn from(ev: SecondLayerEvent) -> Self {
		Self::SecondLayer(ev)
	}
}
impl From<LayerEvent> for PhysicalLayerEvent {
	fn from(ev: LayerEvent) -> Self {
		match ev {
            LayerEvent::PhysicalLayer(ev) => ev,
            _ => PhysicalLayerEvent::None,
        }
	}
}
impl From<LayerEvent> for FirstLayerEvent {
	fn from(ev: LayerEvent) -> Self {
		match ev {
            LayerEvent::FirstLayer(ev) => ev,
            _ => FirstLayerEvent::None,
        }
	}
}
impl From<LayerEvent> for SecondLayerEvent {
	fn from(ev: LayerEvent) -> Self {
		match ev {
            LayerEvent::SecondLayer(ev) => ev,
            _ => SecondLayerEvent::None,
        }
	}
}

#[derive(Clone)]
pub enum LayerPipeItem {
    SomeCommand(LineId, LayerCommand),
    SomeEvent(LineId, LayerEvent),
    None,
}

impl LayerPipeItem {
    pub fn as_cmd(self) -> Option<LayerCommand> {
        match self {
            Self::SomeCommand(_, cmd) => Some(cmd),
            _ => Option::<LayerCommand>::None,
        }
    }

    pub fn as_event(self) -> Option<LayerEvent> {
        match self {
            Self::SomeEvent(_, ev) => Some(ev),
            _ => Option::<LayerEvent>::None,
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            Self::None
            |
            Self::SomeCommand(_, LayerCommand::None) 
            |
            Self::SomeEvent(_, LayerEvent::None) => true,
            _ => false,
        }
    }

    pub fn to_lower(&mut self) {
        match self {
            Self::SomeCommand(_, PhysicalLayer(cmd)) => *self = Self::None,
            Self::SomeCommand(lid, FirstLayer(cmd)) => 
                *self = Self::SomeCommand(*lid, PhysicalLayer(cmd.take().into())),
            Self::SomeCommand(lid, SecondLayer(cmd)) => 
                *self = Self::SomeCommand(*lid, FirstLayer(cmd.take().into())),
            _ => *self = Self::None,
        }
    } 

    pub fn to_upper(&mut self) {
        match self {
            Self::SomeEvent(lid, LayerEvent::SecondLayer(ev)) => *self = Self::None,
            Self::SomeEvent(lid, LayerEvent::FirstLayer(ev)) => 
                *self = Self::SomeEvent(*lid, LayerEvent::SecondLayer(ev.take().into())),
            Self::SomeEvent(lid, LayerEvent::PhysicalLayer(ev)) => 
                *self = Self::SomeEvent(*lid, LayerEvent::FirstLayer(ev.take().into())),
            _ => *self = Self::None,
        }
        ;
    } 
}

impl crate::pipe::Item for LayerPipeItem {
    fn dir(&self) -> crate::pipe::PropagationDirecion {
		match &self {
			Self::SomeEvent(_, _) => crate::pipe::PropagationDirecion::Up,
			Self::SomeCommand(_, _) => crate::pipe::PropagationDirecion::Down,
			_ => crate::pipe::PropagationDirecion::None,
		}
	}
}

impl std::fmt::Display for LayerPipeItem {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "{}", "LayerLayerPipeItem::None"),
            Self::SomeCommand(lid, cmd) => write!(f, "line#{}, {}", lid, cmd),
            Self::SomeEvent(lid, ev) => write!(f, "line#{}, {}", lid, ev),
        }
	}
}

impl From<LayerCommand> for LayerPipeItem {
	fn from(cmd: LayerCommand) -> Self {
        Self::SomeCommand(LineId::default(), cmd)
	}
}
impl From<LayerEvent> for LayerPipeItem {
	fn from(ev: LayerEvent) -> Self {
        Self::SomeEvent(LineId::default(), ev)
	}
}

#[derive(Clone)]
pub struct NodeEvent(pub LineId, pub SecondLayerEvent);

impl std::fmt::Display for NodeEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    	write!(f, "({}, {})", self.0, self.1)
	}
}

#[derive(Clone)]
pub struct NodeCommand(pub LineId, pub SecondLayerCommand);

impl std::fmt::Display for NodeCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    	write!(f, "({}, {})", self.0, self.1)
	}
}

impl From<NodeEvent> for LayerPipeItem {
	fn from(ev: NodeEvent) -> Self {
		Self::SomeEvent(ev.0, LayerEvent::SecondLayer(ev.1))
	}
}
impl From<NodeCommand> for LayerPipeItem {
	fn from(cmd: NodeCommand) -> Self {
		Self::SomeCommand(cmd.0, LayerCommand::SecondLayer(cmd.1))
	}
}

impl From<LayerPipeItem> for NodeCommand {
	fn from(item: LayerPipeItem) -> Self {
		match item {
			LayerPipeItem::SomeCommand(lid, LayerCommand::SecondLayer(cmd)) 
				=> Self(lid, cmd),
			_ => Self (LineId::default(), SecondLayerCommand::None),
		}
	}
}

impl From<LayerPipeItem> for NodeEvent {
	fn from(item: LayerPipeItem) -> Self {
		match item {
			LayerPipeItem::SomeEvent(lid, LayerEvent::SecondLayer(ev)) 
				=> Self(lid, ev),
			_ => Self (LineId::default(), SecondLayerEvent::None),
		}
	}
}
