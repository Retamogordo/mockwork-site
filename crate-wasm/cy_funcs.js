function on_start_transmission(node_addr) {
  window.cy.$id(node_addr) .style({
      'background-color': 'red'
    })
}
function output_box(text) {
  self.postMessage( { ev: 'output_terminal', text 
  } );
}

export {on_start_transmission, output_box}