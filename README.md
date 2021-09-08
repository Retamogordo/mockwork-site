 # Mockwork
 ## Asynchronous Network Simulator.
 ### Introduction
 This program presents a simulator of nodes interconnected by physical media.
 Nodes run a protocol stack that facilitate exchange of data between nodes.
 The code runs on WebAssembly which is loaded to a single Web Worker.

[screenshot](https://github.com/Retamogordo/mockwork/tree/master/docs/Screenshot_mockwork.png)

 ### Disclaimer
 This program was written as a part of my effort to learn Rust language.
 The design of this network is not based on any existing real network implementation.
 All terms used here like socket, peer, stream, protocol etc etc should be
 taken in their broad and vague sense.
 Although somewhat complex, this program does not pursuit any practical utility but eat up your CPU when
 running :)
 
 ### Brief introduction to functioning
 
 Nodes are connected by duplex lines which are modeled by only a couple of
 parameters: propagation delay and noise level.
 
 The bandwidth is limited naturally by JS event loop :)
 
 The line delay is simulated by a timer delay.
 
 Each node run its own event loop in which it listens for Events from line
 and Commands from frontend.
 
 These items - Commands and Events are passed through a duplex pipe where
 protocols reside.
 
 Protocols are entities that receive, recognize, process relevant commands and event
 and, possibly, emit futher items until they are consumed on corresponding
 endpoint of the pipe.
 
 For now protocols are layered into three layers: "Physical", "First" and "Second".
 Rusts type system favors natural firewalling of pipe items in such a way that
 each item must be expicitly converted to the adjacent layer item in order
 to be eligible to propagate.
 
 Each protocol has a header which wraps a message to be transmitted, on the other
 line endpoint the headers are stripped off as long as the message bubbles up in 
 the protocol pipe.
 
 At first nodes are unaware of their mates, the only thing they can do 
 is to send a message online using physical layer transmit.
 There is a protocol that broadcasts requests for address map building upon
 receiving acks from other nodes.
 There is a protocol for channel establishing on a line between two adjacent
 nodes, and this channel can serve as a joint for a longer higher layer channel
 between two arbitrary nodes.
 On a "frontend" level a concept of stream is introduced (does not implement
 a formal Rusts stream interface). 
 Each physical line can serve for data transmitting over multiple logical
 streams.
 Streams lazily expire so they can be garbage collected after line failure.
 The protocol stack is somehow scalable, one can implement a new protocol
 introducing events and commands along with relevant business logic and hooking
 it to the protocol stack.
 
 ### User Interface
 is written in plain Javascript and utilizes [Cytoscape](https://cytoscape.org) for network visualization.
 WebAssemly code runs network simulation event loops in asynchronous fashion
 on separate single Web Worker.
 When the network is actively doing something, Web Worker polls its shared state
 to postmessage it to UI thread and present some indication on network activity.

 ### Technical Issues
 Line delay are simulated by a standard (asynchronously awaited) delays which
 present a small but constant time drift which, by cumulative effect leads to a
 significant time divergence very quickly. 
 So there is a scheduling mechanism aiming to solve this issue, which usually
 works fine but in theory (and in practice) causes some messages to be
 sent before time.
 This is a CPU greedy program, after all there are a plenty of event loops
 on top of JS runtime, and while it is UI responsive, it turns to be tricky to achieve
 internal responsiveness ( I consider I failed to get to a satisfactory solution).
 I suspect my protocol implementation is far from being optimal and represents
 a serious bottleneck for the whole story.
 
 ### Browser compatibility
 I used Firefox 89.0 on Linux while developing and tested it a bit with
 Chromium 93
 
 ### Rust Programming Practices
 My first thing in Rust, most likely not best practiced.
 
 ### Deployment Problems
 I lack knowledge regarding deployment process, after two days of
 trying to adopt a foreign very simple travis.yml I abandoned it 
 for deploying it just brutally.
 
 ### Acknowledgements
 to Victor Gavrish for his 
 [rust-wasm-worker-template](https://github.com/VictorGavrish/rust-wasm-worker-template)
 
