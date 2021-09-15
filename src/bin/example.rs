use hacksaw::make_selection;

fn main() {
    let result = make_selection(None).unwrap();
    println!("Selection Result: {:#?}", result);
}
