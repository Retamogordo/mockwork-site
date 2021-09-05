import { FSM } from "./fsm.js" 

var cytoscape = require("cytoscape");
let fcose = require('cytoscape-fcose');
//var cyCanvas = require('cytoscape-canvas');
//cyCanvas(cytoscape); // Register extension

var $ = require( "jquery" );

const worker = new Worker("./worker.js");

var tabFlows;

var setupTabFlow;
var randomRunTabFlow;
var peerChannelTestFlow;

var outputText = ["", "", "", "", ""];
var outputTextIndex = 0;

const nodesNumBox = document.querySelector("#nodesTextbox");
const edgesNumBox = document.querySelector("#edgesTextbox");
const lineDelayMinBox = document.querySelector("#lineDelayMinBox");
const lineDelayMaxBox = document.querySelector("#lineDelayMaxBox");
const nodeLoopCyclesTickDelaySlider = document.querySelector("#nodeLoopCyclesTickDelay");
const sessionsLimitSlider = document.querySelector("#sessionsLimit");

const createButton = document.querySelector("#createButton");
//const addressMapButton = document.querySelector("#addressMapButton");
//const displayNodeAddressMapButton = document.querySelector("#displayNodeAddressMapButton");

//var statsCanvas = document.getElementById("statsCanvas");
var cy;

var currentTab;
//var currentMode;
var nodesNum;
var edgesNum;
var minLineDelay;
var maxLineDelay;
var nodeLoopCyclesTickDelay;
var overallTraffic = 0;
var trafficSamples = {};

nodesNumBox.addEventListener("mousedown", ev => ev.stopPropagation());
edgesNumBox.addEventListener("mousedown", ev => ev.stopPropagation());
lineDelayMinBox.addEventListener("mousedown", ev => ev.stopPropagation());
lineDelayMaxBox.addEventListener("mousedown", ev => ev.stopPropagation());
document.querySelector("#sessionsLimit").addEventListener("mousedown", ev => ev.stopPropagation());
document.querySelector("#nodeLoopCyclesTickDelay").addEventListener("mousedown", ev => ev.stopPropagation());
document.querySelector("#addressMapSeeds").addEventListener("mousedown", ev => ev.stopPropagation());

worker.addEventListener("message", ev => {
  const message = ev.data;

  if (message.mock_work_loaded !== undefined) {
	randomRunTabFlow.reset();
	peerChannelTestFlow.reset();
	setupTabFlow.loadInstance(message.mock_work_loaded);

	return;
  }

  switch (message.ev) {
	case 'displayNodeAddressMap': {
		displayNodeAddressMap(message);
		break;
	}
/*	
	case 'toggleNodeConnection': {
		cy.$id(message.src).data({connected: message.connected});
		let connected = cy.$id(message.src).data()['connected'];

		cy.$id(message.src).style({
			'background-color': 
				connected ? 'grey' : 'red'
		});
		break;
	}
*/
	case 'pollResult': {
		if (message.iteration % 100 == 0) {
//			console.log(performance.memory);
//			console.log(message.pollResult["current_transmit_info"])
//			console.log("being transmitted: ", message.pollResult["being_transmitted"])
/*				console.log("traffic: " + statsBarItems["traffic"] + ", active streams: " 
			+ statsBarItems["active_streams"]
			+ ", total streams: " + message.pollResult["total_streams"]
			+ ", failed streams: " + message.pollResult["failed_streams"]
			+ ", overall outgoing: " + overall_outgoing
			+ ", overall incoming: " + overall_incoming
			);*/
		}

		if (!message.pollingStopped) 
			currentTab.pollSampleReady && currentTab.pollSampleReady(message.pollResult);
		else {
			currentTab.currentOpStopped && currentTab.currentOpStopped(message.pollResult);
//			console.log("polling stopped");

		}
		break;
	}

	case 'pollError': {
//		currentTab.pollError && currentTab.pollError(message.pollError);
		currentTab.currentOpStopped && currentTab.currentOpStopped(message.pollResult);
/*		worker.postMessage({
			cmd: "shutdownInstance", 
		});*/
		break;
	}

	case 'startPeerChannelRun': {
		document.getElementById("comments").innerHTML = message.src + " -> " + message.dest;	
		break;
	}
	case 'output_terminal': {
		console.log(message.text);
		console.log(outputText);
		if (outputTextIndex < 4) {
			outputText[outputTextIndex] = message.text;
		} else {
			outputText[0] = outputText[1];
			outputText[1] = outputText[2];
			outputText[2] = outputText[3];
			outputText[3] = outputText[4];
			outputText[4] = message.text;
		}
		let text = outputText[0] + outputText[1] 
			+ outputText[2] + outputText[3] + outputText[4];
		outputTextIndex++;
		outputTextIndex = outputTextIndex > 4 ? 4 : outputTextIndex;

		document.getElementById('output_terminal').value = text;
		console.log("OUTPUT BOX in main");

		break;
	}
}
  

});

createButton.addEventListener("click", 
	() => {
		nodesNum = nodesNumBox.value;
		edgesNum = edgesNumBox.value;
		minLineDelay = lineDelayMinBox.value;
		maxLineDelay = lineDelayMaxBox.value;
		
		worker.postMessage({
			cmd: "createInstance", 
			nodesNum, 
			edgesNum,
			minLineDelay,
			maxLineDelay,
			nodeEventLoopTickDelay: nodeLoopCyclesTickDelaySlider.value,
			sessionsLimit: sessionsLimitSlider.value,
		} );
	});
	
function setDefaultCyStyle(cy) {
	cy.style( [ // the stylesheet for the graph
	    {
            selector: 'node',
            style: {
              'background-color': '#red',
			  'width': 10,
			  'height': 10,
            },
        },

	    {
	      selector: 'edge',
	      style: {
	        'width': 3,
	        'line-color': 'hsl(90, 100%, 5%)',
//	        'target-arrow-color': '#ccc',
//			'label': 'data(propagation_delay)',
			//	        'target-arrow-shape': 'triangle',
	        'curve-style': 'unbundled-bezier',
			'control-point-distances': '-20 20 -20',
//			'control-point-weights': '0.25 0.5 0.75'
	      }
	    }
	  ]);

	  cy.nodes()
	  .filter("node[connected]")
	  .style({
		  'background-color': 'hsl(60, 100%, 5%)',
		  'label': '',

		  });
		cy.nodes()
		.filter("node[!connected]")
		.style({
			'background-color': 'red',
			'label': '',

			});
			
		cy.nodes().forEach( node => {
				randomRunTabFlow.currentCyStyle[node.id()] = {
					color: node.data()['connected'] ? 'hsl(60, 100%, 5%)' : 'red',
					size: 10
				}
				peerChannelTestFlow.currentCyStyle[node.id()] = {
					color: node.data()['connected'] ? 'hsl(60, 100%, 5%)' : 'red',
					size: 10
				}
			}
		);

 /* 
	cy.edges()
		.style({
			'line-color': '#ccc',
			'width': 5,
		});*/
		
}

function displayNodeAddressMap(msg) {
//	let total_streams = tmp['total_streams'];
//	setDefaultCyStyle(cy);
	currentTab.refreshCyStyle();

	for (const node_id of Object.keys(msg.address_map)) {
		const line_id = msg.address_map[node_id];

		const line = cy.$id(line_id);
		let target = cy.$id(node_id);

		target.style({
			'background-color': 'cyan',
			'color': 'cyan',
			'label': node_id,
		  });
	}
	let src = cy.$id(msg.src);
	let srcLuminance = (5+20*asympthoticScale(Object.keys(msg.address_map).length, 1.2, 100-5, 50)).toString();
//	let srcColor = 'hsl(300, 100%, ' + srcLuminance + '%)';	
//	let srcColor = blueviolet;	
	src.style({
		'background-color': 'blueviolet',
		'color': 'blueviolet',
		'label': msg.src,
	});

	document.getElementById("comments").innerHTML = "address map for node " + msg.src;
}

function asympthoticScale(value, powBase, limit, scale) {
	return limit*(1 - Math.pow(powBase, -value/scale))
}


$(function() {
	setupTabFlow = new SetupTabFlow();
	randomRunTabFlow = new RandomRunTabFlow();
	peerChannelTestFlow = new PeerChannelTestFlow();

	tabFlows = [setupTabFlow, randomRunTabFlow, peerChannelTestFlow];
	
	setupTabFlow.init();

	setCurrentTab(setupTabFlow.id());


//	document.getElementById("setup").click(); 
//	$("#setupTabButton").on("click", "setupTab", setCurrentTab);
//	$("#runTabButton").on("click", null, setCurrentTab("runTab"));

	document.querySelector("#setupTabButton").addEventListener("click", 
		ev => {
			setCurrentTab("setupTab");
		}
	);
	document.querySelector("#runTabButton").addEventListener("click", 
		ev => {
			setCurrentTab("runTab");
		}
	);
	document.querySelector("#peerChannelTabButton").addEventListener("click", 
		ev => {
			setCurrentTab("peerChannelRunTab");
		}
	);

	document.querySelector("#nodeLoopCyclesTickDelay").addEventListener("change", 
		ev => {
			nodeLoopCyclesTickDelay = ev.target.value;
			worker.postMessage({cmd: 'setMWLoopCycleTickDelay', 
				params: {'nodeLoopCyclesTickDelay': nodeLoopCyclesTickDelay}
			});
		}
	);

	document.querySelector("#sessionsLimit").addEventListener("change", 
		ev => {
			sessionsLimit = ev.target.value;
			worker.postMessage({cmd: 'setMWMaxRunningSessions', 
				params: {'sessionsLimit': sessionsLimit}
			});
		}
	);

});
   

SetupTabFlow.initSignalId = 100;
SetupTabFlow.instanceLoadedSignalId = 101;

function SetupTabFlow() {
	const initApp = () => { 
		setTabExclusiveAccess(this);

		cytoscape.use(fcose);
		cy = cytoscape({
			container: document.getElementById('cy'), // container to render in
		  });
	  		
		cy.on('tap', 'node', function(evt){
			var node = evt.target;
			currentTab.cyNodeTap && currentTab.cyNodeTap(node.id());
		});
		window.cy = cy; 
	}

	const instanceLoaded = (mwJSON, state) => { 
		document.getElementById("output_terminal").value = "";

		let mw;
		try {
			mw = JSON.parse( mwJSON );
			cy.elements().remove(); cy.add( mw );
			setDefaultCyStyle(cy);
			cy.layout({
				name: 'fcose',
				backgroundColor: 'black',
				// Node repulsion (non overlapping) multiplier
				nodeRepulsion: 400000,
				// Node repulsion (overlapping) multiplier
				nodeOverlap: 10,
				// Ideal edge (non nested) length
				idealEdgeLength: function(edge) {
		//		console.log(edge.data());
					return edge.data()['propagation_delay'];
				},
				// Divisor to compute edge forces
				edgeElasticity: 1,
				// Nesting factor (multiplier) to compute ideal edge length for nested edges
				nestingFactor: 5,
				// Gravity force (constant)
				gravity: 80,
			}).run();
	
			overallTraffic = 0;
			resetDynStats();
			enableTabs();
		}
		catch {
		}
	}

	this.fsm = new FSM();
	let fsm = this.fsm;

	fsm.init( {
		initState: { id: 10, description:"before mockwork loaded."},	
		instanceReadyState: { id: 20, description: "instance ready."},	
	});

	fsm.awaiting
		.chain(SetupTabFlow.initSignalId, fsm.initState, initApp)
		.chain(SetupTabFlow.instanceLoadedSignalId, fsm.instanceReadyState, instanceLoaded)
		.chain(SetupTabFlow.instanceLoadedSignalId, fsm.instanceReadyState, instanceLoaded);

	fsm.run();
}

SetupTabFlow.prototype.id = function () { return 'setupTab' }

SetupTabFlow.prototype.init = function () {
	this.fsm.inputSignal({id: SetupTabFlow.initSignalId})
}

SetupTabFlow.prototype.loadInstance = function (mwJSON) {
	this.fsm.inputSignal({id: SetupTabFlow.instanceLoadedSignalId, payload: mwJSON})
}

SetupTabFlow.prototype.onEnter = function () {
	document.getElementById("loops_params_controls").style.display = "none";
	document.getElementById("stats_bars").style.display = "none";
}

SetupTabFlow.prototype.disable = function() {
	document.getElementById("setupTabButton").disabled = true;
}
SetupTabFlow.prototype.enable = function() {
	document.getElementById("setupTabButton").disabled = false;
}

RandomRunTabFlow.warmUpSignalId = 102;
RandomRunTabFlow.runRandomLoopSignalId = 103;
RandomRunTabFlow.stopCurrentOpSignalId = 110;
RandomRunTabFlow.currentOpStoppedSignalId = 111;
RandomRunTabFlow.cyNodeTapSignalId = 105;
//RandomRunTabFlow.toggleNodeConnectionId = 120;

function RandomRunTabFlow() {
	var currentCyStyle;

	stopCurrentOperationButton.addEventListener("click", () => {
		this.stopCurrentOp();
	});
	startRandomLoopRunButton.addEventListener("click", () => {
		this.runRandomLoop();
	});
	addressMapButton.addEventListener("click", 
	() => {
		let seeds = document.querySelector("#addressMapSeeds").value;
		let params = {autoStopMs: 3000, seeds};
		this.warmUp(params);
	});


	const transitionFailure = (failureReason) => { return () => { throw failureReason; } } 
	const stoppingCurrentOp = (op, state) => { 
		worker.postMessage({cmd: "stopCurrentOperation", op: state.op});
//		toggleDisplayNodeMapMode(cy);
	}

	const currentOpStopped = (pollSample, state) => { 
		console.log("RandomRunTabFlow.prototype.currentOpStopped");

//		drawDynStats(0, overallTraffic, 0, {}, 0);
		if (pollSample !== undefined) {
			onPollTrafficSample(this, pollSample, state);
			onPollTrafficSample(this, pollSample, state);
		}
		this.setCyNodeStyles(state);
		this.toggleDisplayNodeMapMode(cy);
		enableTabs();
	}
	
	const warmingUpPollSample = (pollSample, state) => { 
		onPollTrafficSample(this, pollSample, state);
		onPollAddressMapSample(this, pollSample, state);
	}

	const runRandomLoopSample = (pollSample, state) => { 
		onPollTrafficSample(this, pollSample, state);
		onPollAddressMapSample(this, pollSample, state);
	}

	const startWarmingUp = (params, state) => { 
/*		console.log("startWarmingUp, params: ", params);
		document.getElementById("addressMapButton").disabled = true;
		document.getElementById("startRandomLoopRun").disabled = true;
		document.getElementById("stopCurrentOperationButton").disabled = false;
*/

		setTabExclusiveAccess(this);
		worker.postMessage({cmd: "addressMap", params})
	}

	const runningRandomLoop = (_, state) => { 
		console.log("runningRandomLoop");

//		setDefaultCyStyle(cy);
//		document.body.style.cursor = 'arrow';

		addressMapButton.disabled = true;
		startRandomLoopRunButton.disabled = true;
		stopCurrentOperationButton.disabled = false;
	//	document.getElementById("sessionsLimit").style.display = "block";
	//	document.getElementById("nodeLoopCyclesTickDelay").style.display = "block";
		setTabExclusiveAccess(this);	

		let nodeLoopCyclesTickDelay = document.querySelector("#nodeLoopCyclesTickDelay").value;
		let sessionsLimit = document.querySelector("#sessionsLimit").value;
		let params = {
			'nodeLoopCyclesTickDelay': nodeLoopCyclesTickDelay,
			'sessionsLimit': sessionsLimit,
		}
	
		worker.postMessage({cmd: "startInfiniteRun", params});	
	}
	
	const cyNodeTap = (nodeId, state) => { 
//		console.log(appFlow.currentTab().nodeTapCommand + ' ' + nodeId);

		worker.postMessage( {cmd: state.nodeTapCommand,  node_id: nodeId } );
//		worker.postMessage( {cmd: currentMode["nodeTapCommand"],  node_id: nodeId } );
	}

	this.fsm = new FSM();
	let fsm = this.fsm;

	fsm.init( {
//		initState: { id: 10, description:"before mockwork loaded."},	
//		instanceReadyState: { id: 20, description: "instance ready."},	
		warmingUpState: { 
			id: 30, 
			op: 'addressMap', 
			description: "warming up.",
			nodeTapCommand: 'displayNodeAddressMap',
//			cyNodeStyles: [],
		},	
		waitingForFutherCommandState: { 
			id: 33,
			nodeTapCommand: 'displayNodeAddressMap', 
			description: "warmed up.",
		},	
		
		runningRandomLoopState: { 
			id: 40,
			op: 'randomRun',
			nodeTapCommand: 'toggleNodeConnection',  
//			overall_outgoing: 0,
//			overall_incoming: 0,
		},	
//failure: { id: 55, description: "state transition failure.", on: onFailure},	
	});

	fsm.awaiting.nodeTapCommand = 'displayNodeAddressMap';

	fsm.awaiting
		.chain(RandomRunTabFlow.warmUpSignalId, fsm.warmingUpState, startWarmingUp)
		.chain(RandomRunTabFlow.pollSampleSignalId, fsm.warmingUpState, warmingUpPollSample)
		.chain(RandomRunTabFlow.stopCurrentOpSignalId, fsm.warmingUpState, stoppingCurrentOp)
		.chain(RandomRunTabFlow.currentOpStoppedSignalId, fsm.waitingForFutherCommandState, currentOpStopped)
		.chain(RandomRunTabFlow.runRandomLoopSignalId, fsm.runningRandomLoopState, runningRandomLoop)
		.chain(RandomRunTabFlow.stopCurrentOpSignalId, fsm.runningRandomLoopState, stoppingCurrentOp )		
//		.chain(RandomRunTabFlow.toggleNodeConnectionId, fsm.runningRandomLoopState, toggleNodeConnection )		
		.chain(RandomRunTabFlow.currentOpStoppedSignalId, fsm.waitingForFutherCommandState, currentOpStopped)
	
	fsm.runningRandomLoopState
		.chain(RandomRunTabFlow.pollSampleSignalId, fsm.runningRandomLoopState, runRandomLoopSample)
		.chain(RandomRunTabFlow.cyNodeTapSignalId, fsm.runningRandomLoopState, cyNodeTap)

	fsm.warmingUpState
		.chain(RandomRunTabFlow.cyNodeTapSignalId, fsm.warmingUpState, cyNodeTap)
		
	fsm.waitingForFutherCommandState
		.chain(RandomRunTabFlow.cyNodeTapSignalId, fsm.waitingForFutherCommandState, cyNodeTap)
		.chain(RandomRunTabFlow.warmUpSignalId, fsm.warmingUpState, startWarmingUp);

	fsm.awaiting
		.chain(RandomRunTabFlow.cyNodeTapSignalId, fsm.awaiting, cyNodeTap);

	fsm.run();
//	fsm.stop();
}

RandomRunTabFlow.prototype.reset = function () {
	this.fsm.stop();
	this.fsm.run();
	this.currentCyStyle = [];

	this.toggleDefaultDisplayMode(cy);
}

RandomRunTabFlow.prototype.onEnter = function () {
	document.getElementById("loops_params_controls").style.display = "block";
	document.getElementById("stats_bars").style.display = "block";
	document.getElementById("sessionsLimit").style.display = "block";
}

RandomRunTabFlow.prototype.runRandomLoop = function () {
	this.fsm.inputSignal({id: RandomRunTabFlow.runRandomLoopSignalId})
}

RandomRunTabFlow.prototype.warmUp = function (params) {
	this.fsm.inputSignal({id: RandomRunTabFlow.warmUpSignalId, payload: params})
}

RandomRunTabFlow.prototype.stopCurrentOp = function() {
	this.fsm.inputSignal({id: RandomRunTabFlow.stopCurrentOpSignalId})
}

RandomRunTabFlow.prototype.currentOpStopped = function(pollSample) {
	this.fsm.inputSignal({id: RandomRunTabFlow.currentOpStoppedSignalId, payload: pollSample})
}

RandomRunTabFlow.prototype.pollSampleReady = function(pollSample) {
	this.fsm.inputSignal({id: RandomRunTabFlow.pollSampleSignalId, payload: pollSample})
}

RandomRunTabFlow.prototype.cyNodeTap = function(nodeId) {
	this.fsm.inputSignal({id: RandomRunTabFlow.cyNodeTapSignalId, payload: nodeId})
}

RandomRunTabFlow.prototype.refreshCyStyle = function() {
	this.setCyNodeStyles();
}

RandomRunTabFlow.prototype.id = function () { return 'runTab' }

RandomRunTabFlow.prototype.toggleDefaultDisplayMode = function (cy) {
	addressMapButton.disabled = false;
	startRandomLoopRunButton.disabled = true;
	stopCurrentOperationButton.disabled = true;

	sessionsLimitSlider.disabled = true;
	nodeLoopCyclesTickDelaySlider.disabled = true;

	document.body.style.cursor = 'arrow';
}

RandomRunTabFlow.prototype.toggleDisplayNodeMapMode = function(cy) {
	addressMapButton.disabled = false;
	startRandomLoopRunButton.disabled = false;
	stopCurrentOperationButton.disabled = true;
	sessionsLimitSlider.disabled = false;
	nodeLoopCyclesTickDelaySlider.disabled = false;

	document.body.style.cursor = 'crosshair';
}

RandomRunTabFlow.prototype.setCyNodeStyles = function(state) {
	let cyNodeStyles = this.currentCyStyle;
	Object.keys(cyNodeStyles).forEach( (nodeId) => {
//		Object.keys(cyNodeStyles).forEach( (nodeId) => {
		if (cy.$id(nodeId).data()['connected']) {
			cy.$id(nodeId)
				.style( { 
					'background-color': cyNodeStyles[nodeId].color,
					'width': cyNodeStyles[nodeId].size,
					'height': cyNodeStyles[nodeId].size,
					'label': '',

				} );
		} else {
			cy.$id(nodeId)
				.style( { 
					'background-color': 'red',
					'label': '',

				} );
		}
	})
}

RandomRunTabFlow.prototype.disable = function() {
	document.getElementById("runTabButton").disabled = true;
}
RandomRunTabFlow.prototype.enable = function() {
	document.getElementById("runTabButton").disabled = false;
}


PeerChannelTestFlow.cyNodeTapSignalId = 101;
PeerChannelTestFlow.pollSampleSignalId = 102;
PeerChannelTestFlow.stopCurrentOpSignalId = 110;
PeerChannelTestFlow.currentOpStoppedSignalId = 111;
PeerChannelTestFlow.leaveSignalId = 120;

function PeerChannelTestFlow() {
	var currentCyStyle;

	stopPeerOperationButton.addEventListener("click", () => {
		console.log("stopPeerOperationButton clicked");
		this.stopCurrentOp();
	});

	const firstNodeTapped = (nodeId, state) => {
		this.setCyNodeStyles(state);

		cy.$id(nodeId).style({
			'background-color': 'magenta',
			'color': 'magenta',
			'label': nodeId,
		  });

		return nodeId;
	}

	const secondNodeTapped = (dest, state) => {
		setTabExclusiveAccess(this);
		if (state.nodeId == dest) {
			this.setCyNodeStyles(state);
			throw 'will not change state when src == dest';
		} else {
			
			cy.$id(dest).style({
				'background-color': 'magenta',
				'color': 'magenta',
				'label': dest,
			  });

			let nodeLoopCyclesTickDelay = document.querySelector("#nodeLoopCyclesTickDelay").value;
			let sessionsLimit = document.querySelector("#sessionsLimit").value;
			worker.postMessage({cmd: 'runPeerTest', 
				src: state.nodeId,
				dest,
				params: {
					'nodeLoopCyclesTickDelay': nodeLoopCyclesTickDelay,
					'sessionsLimit': sessionsLimit
				}
			});
		}
	}
	const nodeTappedWhenRunning = (nodeId, state) => {
//		this.setCyNodeStyles(currentCyStyle);
		let nodeLabel = nodeId;
		if (state.nodeTapped) {
			if (state.nodeTapped != nodeId) {
				worker.postMessage({cmd: 'injectPeerTest', 
					src: state.nodeTapped,
					dest: nodeId
				});
			} else {
				nodeLabel = ''; 
			}

			state.nodeTapped = undefined;
		}
		else state.nodeTapped = nodeId;

		cy.$id(nodeId).style({
			'background-color': 'magenta',
			'color': 'magenta',
			'label': nodeLabel,
		  });

		return nodeId;
	}

	const stoppingCurrentOp = (op, state) => { 
		console.log("PeerChannelTestFlow.prototype.stoppingCurrentOperation");
		worker.postMessage({cmd: "stopCurrentOperation", op: state.op});

//		toggleDisplayNodeMapMode(cy);
	}

	const pollSample = (pollSample, state) => {
		onPollTrafficSample(this, pollSample, state);
		onPollAddressMapSample(this, pollSample, state);
	}

	const currentOpStopped = (pollSample, state) => { 
		console.log("PeerChannelTestFlow.prototype.currentOpStopped");

//		drawDynStats(0, overallTraffic, 0, {}, 0);
	if (pollSample !== undefined) {
		onPollTrafficSample(this, pollSample, state);
		onPollTrafficSample(this, pollSample, state);
	}
	this.setCyNodeStyles(false);
		enableTabs();
	}

	const onLeave = (_, state) => { 
		this.setCyNodeStyles(false);
	}

	this.fsm = new FSM();
	let fsm = this.fsm;

	fsm.init( {
		cyFirstNodeTappedState: { 
			id: 10, 
			cyNodeStyles: [],
			withLabels: true,
			on: (_, state, nodeId) => { 
				state.nodeId = nodeId 
				document.getElementById("stopPeerOperationButton").disabled = true;
			}
		},
		runningState: {
			id: 20,
			op: 'peerChannelRun', 
			withLabels: true,
			nodeTapped: undefined,
			on: (_, state, nodeId) => { 
				document.getElementById("stopPeerOperationButton").disabled = false;
			}
			

		},	
		stoppingState: {
			id: 30,
			op: 'peerChannelRun',
			withLabels: false, 
			nodeTapped: undefined,
			on: (_, state, nodeId) => { 
				document.getElementById("stopPeerOperationButton").disabled = true;
			}

		},	
	});

	fsm.awaiting
		.chain(PeerChannelTestFlow.cyNodeTapSignalId, fsm.cyFirstNodeTappedState, firstNodeTapped)
		.chain(PeerChannelTestFlow.cyNodeTapSignalId, fsm.runningState, secondNodeTapped)
		.chain(PeerChannelTestFlow.pollSampleSignalId, fsm.runningState, pollSample)
		.chain(PeerChannelTestFlow.cyNodeTapSignalId, fsm.runningState, nodeTappedWhenRunning)
		.chain(PeerChannelTestFlow.stopCurrentOpSignalId, fsm.stoppingState, stoppingCurrentOp)
		.chain(PeerChannelTestFlow.currentOpStoppedSignalId, fsm.awaiting, currentOpStopped)
		.chain(PeerChannelTestFlow.leaveSignalId, fsm.awaiting, onLeave)
	fsm.runningState
		.chain(PeerChannelTestFlow.currentOpStoppedSignalId, fsm.awaiting, currentOpStopped)
/*
	.chain(PeerChannelTestFlow.cyNodeTapSignalId, fsm.runningWithNodeTappedState, nodeTappedWhenRunning)
		.chain(PeerChannelTestFlow.pollSampleSignalId, fsm.runningWithNodeTappedState, pollSample)
		.chain(PeerChannelTestFlow.stopCurrentOpSignalId, fsm.runningState, stoppingCurrentOp)
	fsm.runningWithNodeTappedState
		.chain(PeerChannelTestFlow.cyNodeTapSignalId, fsm.runningState, nodeTappedWhenRunning)
*/

	fsm.run();
}

PeerChannelTestFlow.prototype.id = function () { return 'peerChannelRunTab' }

PeerChannelTestFlow.prototype.reset = function () {
	this.currentCyStyle = [];

	this.fsm.stop();
	this.fsm.run();
}

PeerChannelTestFlow.prototype.cyNodeTap = function (nodeId) { 
	this.fsm.inputSignal({id: PeerChannelTestFlow.cyNodeTapSignalId, payload: nodeId})
}

PeerChannelTestFlow.prototype.pollSampleReady = function(pollSample) {
	this.fsm.inputSignal({id: PeerChannelTestFlow.pollSampleSignalId, payload: pollSample})
}

PeerChannelTestFlow.prototype.stopCurrentOp = function() {
	this.fsm.inputSignal({id: PeerChannelTestFlow.stopCurrentOpSignalId})
}

PeerChannelTestFlow.prototype.currentOpStopped = function(pollSample) {
	this.fsm.inputSignal({id: PeerChannelTestFlow.currentOpStoppedSignalId, payload: pollSample})
}

PeerChannelTestFlow.prototype.onLeave = function() {
	this.fsm.inputSignal({id: PeerChannelTestFlow.leaveSignalId})
}

PeerChannelTestFlow.prototype.onEnter = function() {
	document.getElementById("loops_params_controls").style.display = "block";
	document.getElementById("sessionsLimit").style.display = "none";
	document.getElementById("sessionsLimitLabel").style.display = "none";
	document.getElementById("stats_bars").style.display = "block";
	document.getElementById("comments").innerHTML = "Tap on a pair of nodes to run transfers";	

	sessionsLimitSlider.disabled = false;
	nodeLoopCyclesTickDelaySlider.disabled = false;

	document.body.style.cursor = 'crosshair';

	this.setCyNodeStyles(this.fsm.awaiting);
}

PeerChannelTestFlow.prototype.setCyNodeStyles = function(state) {
	let cyNodeStyles = this.currentCyStyle;
	Object.keys(cyNodeStyles).forEach( (nodeId) => {
//		Object.keys(cyNodeStyles).forEach( (nodeId) => {
		if (cy.$id(nodeId).data()['connected']) {
			cy.$id(nodeId)
				.style( { 
					'background-color': cyNodeStyles[nodeId].color,
					'width': cyNodeStyles[nodeId].size,
					'height': cyNodeStyles[nodeId].size,
//					'label': '',

				} );
		} else {
			cy.$id(nodeId)
				.style( { 
					'background-color': 'red',
//					'label': '',
				} );
		}
		if (!state || !state.withLabels)
			cy.$id(nodeId)
				.style( { 
					'label': '',

			} );
	})
}

PeerChannelTestFlow.prototype.disable = function() {
	document.getElementById("peerChannelTabButton").disabled = true;
}
PeerChannelTestFlow.prototype.enable = function() {
	document.getElementById("peerChannelTabButton").disabled = false;
}

function setCurrentTab(tabId) {
	var i, tabcontent, tablinks;
	let nodeTapCommand;

	if (currentTab === undefined || currentTab.id() != tabId) {
		currentTab && currentTab.onLeave && currentTab.onLeave();

		switch (tabId) {
			case 'setupTab': {
				currentTab = setupTabFlow;
				break;
			}
			case 'runTab': {
				currentTab = randomRunTabFlow;
	//			nodeTapCommand = 'displayNodeAddressMap';
				break;
			}
			case 'peerChannelRunTab': {
				currentTab = peerChannelTestFlow;
//				currentTab.onEnter();
				break;
			}
		}
		currentTab && currentTab.onEnter && currentTab.onEnter();

	}

  	tabcontent = document.getElementsByClassName("tabcontent");
  	for (i = 0; i < tabcontent.length; i++) {
    	tabcontent[i].style.display = "none";
 	}
	document.getElementById(tabId).style.display = "block";
}

function setTabExclusiveAccess(thisTabFlow) {
	for (let tabFlow of tabFlows) {
		if (tabFlow != thisTabFlow) {
			tabFlow.disable();
		}
	}
}

function enableTabs() {
	for (let tabFlow of tabFlows) {
		tabFlow.enable();
	}
}

function resetDynStats() {
	
	let sessionsRunningStatsBar = document.getElementById('sessionsRunningStatsBar');
	sessionsRunningStatsBar.style.width = '0%';
	sessionsRunningStatsBar.innerHTML = "0";

	let currentTrafficStatsBar = document.getElementById('currentTrafficStatsBar');
	let overallTrafficStatsBar = document.getElementById('overallTrafficStatsBar');
	currentTrafficStatsBar.style.width = "0%";
	currentTrafficStatsBar.innerHTML = "0";
	
	overallTrafficStatsBar.style.width = "0%";
	overallTrafficStatsBar.innerHTML = "0";
}

function drawDynStats(
	total_current_traffic, 
	overall_traffic,
	remaining_scheduled,
	trafficDeltas, 
	sessions_running
) {
	
	Object.keys(trafficDeltas).forEach( (line) => {
		let sample = trafficDeltas[line];
		let lineColor = 'hsl(90, 100%, ' + (5+asympthoticScale(sample, 1.2, 100-5, 50)).toString() + '%)';

		cy.$id(line)
			.style( { 'line-color': lineColor} );
	});
	

	let widthLimit = 100;

	let w = asympthoticScale(sessions_running, 1.2, widthLimit, 1);
	let sessionsRunningStatsBar = document.getElementById('sessionsRunningStatsBar');
	sessionsRunningStatsBar.style.width = Math.floor(w).toString() + '%';
	sessionsRunningStatsBar.innerHTML = sessions_running.toString();

	let currentTrafficStatsBar = document.getElementById('currentTrafficStatsBar');
	let overallTrafficStatsBar = document.getElementById('overallTrafficStatsBar');
//	let peerCurrentTrafficStatsBar = document.getElementById('peerCurrentTrafficStatsBar');
	
	w = asympthoticScale(total_current_traffic, 1.2, widthLimit, 50);
	currentTrafficStatsBar.style.width = Math.floor(w).toString() + "%";
	currentTrafficStatsBar.innerHTML = total_current_traffic.toString();
	
	w = asympthoticScale(overallTraffic, 1.01, widthLimit, 5);
	overallTrafficStatsBar.style.width = Math.floor(w).toString() + "%";
	overallTrafficStatsBar.innerHTML = overall_traffic.toString() + "+" + remaining_scheduled.toString();

//	peerCurrentTrafficStatsBar.style.width = Math.floor(w).toString() + "%";
//	peerCurrentTrafficStatsBar.innerHTML = total_current_traffic.toString();
}

function onPollTrafficSample(tabFlow, pollSample, state) {
	
	let total_current_traffic = pollSample["current_transmit_info"]["overall_transmitted"]
		- overallTraffic;
	overallTraffic = pollSample["current_transmit_info"]["overall_transmitted"];
	let remaining_scheduled = pollSample["current_transmit_info"]["remaining_scheduled"];

	let samples = pollSample["current_transmit_info"]["traffic_samples"];
	let trafficDeltas = {};
	
	Object.keys(samples).forEach( line => {
	
		trafficDeltas[line] = trafficSamples[line] !== undefined ? 
			samples[line][0] - trafficSamples[line] : samples[line][0];
		trafficSamples[line] = samples[line][0];
	});
	
//	console.log(trafficDeltas);
	let sessions_running = pollSample["sessions_running"];

	drawDynStats(total_current_traffic, overallTraffic, remaining_scheduled, trafficDeltas, sessions_running);
}

function onPollAddressMapSample(tabFlow, pollSample, state) {
	
	let node_state = pollSample["active_nodes_map"];

	Object.keys(node_state).forEach( (node_id) => {
		let n = node_state[node_id].neighbours.length;
//		if (n >0)
//			document.getElementById("output_terminal").value += "node " + node_id
//				+ " got acks from " + n + " nodes\n";
		let nodeColor;
		let nodeSize;
		if (node_state[node_id].connected) {
			nodeColor = 'hsl(60, 100%, ' + (5+asympthoticScale(n, 2, 100-5, 7)).toString() + '%)';
			nodeSize = 5 + asympthoticScale(n, 2, 5, 7);
		} else {
			nodeColor = 'red';
			nodeSize = 5;
		}
		tabFlow.currentCyStyle[node_id] = {
			color: nodeColor,
			size: nodeSize
		}
	});
	tabFlow.setCyNodeStyles(state);
}