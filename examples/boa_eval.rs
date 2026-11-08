//! Minimal Servo GTK + Boa embedding smoke test.

use servo_gtk::BoaRuntime;

fn main() {
    let mut runtime = BoaRuntime::new();
    match runtime.eval("const answer = 40 + 2; answer") {
        Ok(value) => println!("Boa result: {value}"),
        Err(error) => {
            eprintln!("Boa error: {error:?}");
            std::process::exit(1);
        }
    }
}
