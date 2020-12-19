use avalanche::{component, reactive_assert, UseState};
use avalanche_web::{Button, Div, Input, Text, H2};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{console, HtmlInputElement};

// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Debug)]
struct Item {
    text: String,
    id: u32,
}

#[component(text = UseState<String>, items = UseState<Vec<Item>>, monotonic_id = UseState<u32>)]
fn Todo() {
    let (text, set_text) = text(String::new());
    let (items, update_items) = items(Vec::new());
    let (monotonic_id, update_monotonic_id) = monotonic_id(0);
    let monotonic_id = *monotonic_id;

    let text_clone = text.clone();
    let set_text_clone = set_text.clone();

    let children = items
        .iter()
        .map(|item| {
            Text! {
                text: "Item ".to_owned() + &item.text,
                key: item.id.to_string()
            }
        })
        .collect::<Vec<_>>();

    reactive_assert!(items => children);

    let items_txt: wasm_bindgen::JsValue = format!("items: {:#?}", items).into();
    console::log_1(&items_txt);

    Div! {
        children: [
            H2!{
                child: Text!{text: "Todo!"},
            },
            Input!{
                value: text.to_owned(),
                on_input: move |e| {
                    let input = e.current_target().unwrap().dyn_into::<HtmlInputElement>().unwrap();
                    set_text.call(|text| *text = input.value());
                }
            },
            Div!{
                children: [
                    Text!{text: "id: "},
                    Text!{text: monotonic_id.to_string()},
                    Text!{text: " text: "},
                    Text!{text: text.clone()}
                ]
            },
            Div!{
                children
            },
            Button!{
                // on_click: move |_| set_count.call(|count| *count += 1),
                child: Text!{text: "Create"},
                on_click: move |_| {
                    let text_clone = text_clone.clone();
                    update_items.call(|items| items.push(Item {
                        text: text_clone,
                        id: monotonic_id
                    }));
                    set_text_clone.call(|text| text.clear());
                    update_monotonic_id.call(|id| *id += 1);
                }
            },
        ]
    }
}

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn main_js() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    //TODO: the App initialization is ugly, provide a Default impl for unit struct components?
    avalanche_web::mount_to_body(
        <<Todo as avalanche::Component>::Builder>::new()
            .build((line!(), column!()))
            .into(),
    );
}
