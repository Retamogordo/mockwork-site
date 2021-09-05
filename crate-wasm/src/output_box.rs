use web_sys::*;
use wasm_bindgen::JsCast;

/*#[wasm_bindgen(module = "/worker.js")]
extern "C" {
    fn output_box();
}
*/
pub fn get_output_box(id: &str, text: &str)
{
/*    output_box();

    let window = web_sys::window().expect("global window does not exists"); 
    let slf = window.self_();   
    let document = window.document().expect("expecting a document on window");
    //let body = document.body().expect("document expect to have have a body");
    let output_box = document.get_element_by_id(id)
    .unwrap()
    .dyn_into::<web_sys::HtmlTextAreaElement>()
    .unwrap();

    output_box.set_value(text);
    */
//    web_sys::console::log_2(&"URL: %s".into(),&JsValue::from_str(&val.inner_text()));
}

