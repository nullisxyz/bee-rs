use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::{Event, HtmlDivElement, HtmlInputElement, HtmlButtonElement};

#[wasm_bindgen(start)]
fn main() -> Result<(), JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();

    // Create a new div element to contain the input and button
    let container_div = document.create_element("div").unwrap().dyn_into::<HtmlDivElement>().unwrap();
    container_div.set_id("container_div");
    document.body().unwrap().append_child(&container_div).unwrap();

    // Create a label for the input area
    let label = document.create_element("label").unwrap();
    label.set_text_content(Some("Input:"));
    container_div.append_child(&label).unwrap();

    // Create an input text area
    let input = document.create_element("input").unwrap().dyn_into::<HtmlInputElement>().unwrap();
    input.set_id("input_text");
    container_div.append_child(&input).unwrap();

    // Create a "Calculate" button
    let button = document.create_element("button").unwrap().dyn_into::<HtmlButtonElement>().unwrap();
    button.set_inner_text("Calculate");
    container_div.append_child(&button).unwrap();

    let closure = Closure::wrap(Box::new(move |_event: Event| {
        // Build a chunk with the input text. The chunk size is 4096 bytes.
        let f = bmt::file::ChunkedFile::new(input.value().as_bytes().to_vec(), bmt::chunk::Options { max_payload_size: 4096 });
        // Display the swarm hash
        window().unwrap().alert_with_message(&format!("Swarm hash: {:#?}", hex::encode(f.address()))).unwrap();
    }) as Box<dyn FnMut(Event)>);

    button.set_onclick(Some(closure.as_ref().unchecked_ref()));

    closure.forget();

    Ok(())
}