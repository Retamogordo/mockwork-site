import("../../crate-wasm/pkg").then(wasm => {
  let wasmInit = wasm.init();
  let mwLoaded = false;
  let src = undefined;
  let dest = undefined;
  let pollInterval = undefined;
  //let pollingActive = false;
  let stoppingPolling = false;
  let nodeLoopCyclesTickDelay = 0;
  let sessionsLimit = 20;
  
  self.addEventListener("message", ev => {
    let msg = ev.data;
    
    if (!wasmInit) {
      return;
    }

    if (msg.cmd == "createInstance") { 
//      if (pollInterval !== undefined) {
        pollInterval = clearInterval(pollInterval);
        //pollingActive = 
        stoppingPolling = false;
//      }
//      console.log(msg.nodesNum);
      let nodesNum = parseInt(msg.nodesNum);
      if (isNaN(nodesNum) || nodesNum < 2) { nodesNum = 2; }

      let edgesNum = parseInt(msg.edgesNum);
      if (isNaN(edgesNum) || edgesNum < 1) { edgesNum = 1; }

      let nodeEventLoopTickDelay = parseInt(msg.nodeEventLoopTickDelay);
      if (isNaN(nodeEventLoopTickDelay) || nodeEventLoopTickDelay < 0 ) { nodeEventLoopTickDelay = 20; }

      let minLineDelay = parseInt(msg.minLineDelay);
      if (isNaN(minLineDelay) || minLineDelay < 1) { minLineDelay = 1; }

      let maxLineDelay = parseInt(msg.maxLineDelay);
      if (isNaN(maxLineDelay) || maxLineDelay < minLineDelay) { maxLineDelay = minLineDelay; }

      wasm.load_instance(
        nodesNum, 
        edgesNum,
        minLineDelay,
        maxLineDelay, 
        nodeEventLoopTickDelay,
      ).then( (mwJson) => {

        if (mwJson !== undefined && mwJson != "") {
          mwLoaded = true;
          self.postMessage({ mock_work_loaded: mwJson });
        }
      } );

      return;
    }

    if (!mwLoaded) {
      return;
    }

    if (msg.cmd == "addressMap") { 
      if (msg.src === undefined) { 
        let autoStopMs = !isNaN(msg.params.autoStopMs) ? msg.params.autoStopMs : 3000; 
        setTimeout( () => {
          stoppingPolling = true;
        }, autoStopMs);

        startPolling();
        
        wasm.network_refresh_address_map(msg.params.seeds, BigInt(msg.params.expiresMs));

   /*     setTimeout( () => {
          let res = JSON.parse( wasm.poll_mock_work("|0.0|") );
          console.log(res);
          self.postMessage( { address_map: res["active_nodes_map"] } )
        }, 1000)*/
      
      }
      else {
//        startPolling();
        wasm.node_refresh_address_map(msg.src, BigInt(10));
      }  
    } 
    else if (msg.cmd == "displayNodeAddressMap") {
//      console.log('displayNodeAddressMap' + msg.node_id);
      let res = JSON.parse(wasm.get_node_address_map(msg.node_id));
      self.postMessage( { ev: "displayNodeAddressMap", address_map: res, src: msg.node_id } )
    }
    else if (msg.cmd == "toggleNodeConnection") {
//      console.log('toggleNodeConnection' + msg.node_id);
      wasm.toggle_node_connection(msg.node_id);
//      self.postMessage( { ev: "toggleNodeConnection", connected: res, src: msg.node_id } )
    }
    else if (msg.cmd == "setMWLoopCycleTickDelay") {
//      console.log('nodeLoopCyclesTickDelay ' + msg.params['nodeLoopCyclesTickDelay']);
      nodeLoopCyclesTickDelay = msg.params['nodeLoopCyclesTickDelay'];
      wasm.set_loop_cycle_gap(nodeLoopCyclesTickDelay);
    }
    else if (msg.cmd == "setMWMaxRunningSessions") {
      sessionsLimit = msg.params['sessionsLimit'];
      wasm.set_max_running_sessions_num(sessionsLimit);
    }
    else if (msg.cmd == "runPeerTest") {
        self.postMessage( { ev: 'startPeerChannelRun', 
          src: msg.src, 
          dest: msg.dest,
        } );

        startPolling();
        wasm.set_loop_cycle_gap(msg.params['nodeLoopCyclesTickDelay']);
        wasm.set_max_running_sessions_num(msg.params['sessionsLimit']);
          wasm.peer_channel_test(msg.src, msg.dest);
    }
    else if (msg.cmd == "injectPeerTest") {
//      console.log("inject peer test ", msg.src, " -> ", msg.dest);
      wasm.peer_channel_test(msg.src, msg.dest);
    }
    else if (msg.cmd == "peerNodeTap") {
      if (src === undefined) {
        src = msg.node_id 
      }
      else {
 
        dest = msg.node_id;    
  
        self.postMessage( { ev: 'startPeerChannelRun', 
          src, 
          dest,
        } );

        startPolling();
        wasm.peer_channel_test(src, dest);

        src = undefined;
      }
    }
    else if (msg.cmd == "pollMockWork") { 
//      let res = JSON.parse( wasm.poll_mock_work(msg.args[0]) );
//      console.log(res['traffic_samples']);
//      self.postMessage( { address_map: res["active_nodes_map"] } );
    }
    else if (msg.cmd == "startInfiniteRun") { 
      nodeLoopCyclesTickDelay = msg.params['nodeLoopCyclesTickDelay'];
      wasm.set_loop_cycle_gap(nodeLoopCyclesTickDelay);
      sessionsLimit = msg.params['sessionsLimit'];
      wasm.set_max_running_sessions_num(sessionsLimit);

      startPolling();
      wasm.set_loop_cycle_gap(msg.params['nodeLoopCyclesTickDelay']);
      wasm.set_max_running_sessions_num(msg.params['sessionsLimit']);
      wasm.random_peer_channel_test();
//      wasm.peer_channel_test("|1.1|", "|1.3|");

  }
    else if (msg.cmd == "stopCurrentOperation") { 
//      console.log("worker stopCurrentOperation cmd, op:", msg.op);
//      stoppingPolling = true;
      if (msg.op == 'randomRun') {
///        console.log("stopping random run");
        wasm.stop_infinite_run();
      }
      else 
      if (msg.op == 'addressMap') {
//        console.log("stopping address map");
        wasm.stop_refreshing_address_map();
      }
      else
      if (msg.op == 'peerChannelRun') {
//        console.log("stopping peer channel run");
        wasm.stop_infinite_run();
      }
    }
    else if (msg.cmd == "shutdownInstance") { 
      wasm.stop_infinite_run();
      wasm.shutdown_instance();
    }
  });

  function startPolling() {
    if (pollInterval === undefined) {
      let iter = 0;
//      pollingActive = true;
      stoppingPolling = false;

      pollInterval = setInterval( () => {
        iter++;
        try {
          let res = JSON.parse( wasm.poll_mock_work(nodeLoopCyclesTickDelay) );
          
          if (res === undefined) {
            return;
          }

  //stoppingPolling |= (res['mw_status'] == "Suspending" || res['mw_status'] == "Suspended");
          stoppingPolling |= (res['was_suspended'] == true);

          if (stoppingPolling) {
              pollInterval = clearInterval(pollInterval);
              stoppingPolling = false;
          }
          
          self.postMessage( { ev: 'pollResult', 
            pollResult: res, 
            iteration: iter,
            pollingStopped: !pollInterval
  //          pollingStopped: !pollingActive
          } )
        }
        catch (error) {
          console.log(error);
          pollInterval = clearInterval(pollInterval);
          self.postMessage( { ev: 'pollError', 
            pollingStopped: true,
  //         error
  //          pollingStopped: !pollingActive
          } )
        }
      }, 50)
    }
  } 
});
