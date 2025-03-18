use skim::prelude::*;
use std::sync::Arc;

fn main() {
    let options = SkimOptionsBuilder::default()
        .multi(false)
        .bind(vec!["enter:accept".to_string()])
        .build()
        .unwrap();

    // Example data (strings)
    let input_lines = vec!["line 1", "line 2", "line 3"];

    // Channel for items
    let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
    for line in &input_lines {
        // SkimItem can be a String (skim auto-implements SkimItem)
        tx_item.send(Arc::new(line.to_string())).unwrap();
    }
    drop(tx_item); // Let Skim know we're done sending

    // Run skim
    if let Some(output) = Skim::run_with(&options, Some(rx_item)) {
        // final_key is the key actually pressed (Enter, Esc, etc.)
        eprintln!("Final key was: {:?}", output.final_key);

        if output.selected_items.is_empty() {
            println!("No items selected.");
        } else {
            println!("Selected items:");
            for item in &output.selected_items {
                println!(" - {}", item.output());
            }
        }
    }
}
