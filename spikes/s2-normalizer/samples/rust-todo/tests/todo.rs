use cucumber::{given, then, when, writer, World as _};

#[derive(Debug, Default, cucumber::World)]
struct TodoWorld {
    list: Vec<String>,
}

#[given("an empty todo list")]
fn empty(w: &mut TodoWorld) {
    w.list.clear();
}

#[when(expr = "I add {string}")]
fn add(w: &mut TodoWorld, item: String) {
    w.list.push(item);
}

// NOTE: no step definition for 'I remove {string}' — on purpose (undefined-step probe).

#[then(expr = "the list contains {int} items")]
fn contains(w: &mut TodoWorld, n: usize) {
    assert_eq!(w.list.len(), n);
}

#[tokio::main]
async fn main() {
    let file = std::fs::File::create("rust-junit.xml").expect("junit file");
    TodoWorld::cucumber()
        .with_writer(writer::JUnit::new(file, 0))
        .run("tests/features")
        .await;
}
