use std::fs;
use std::io::Write;

fn main() {
    let body = fs::read_to_string("challenge_full.html").unwrap();
    let start = body.find("window.onload").unwrap();
    let end = body.rfind("</script>").unwrap();
    let script = &body[start..end];

    for i in 0..8 {
        let s = i * 1000;
        let e = ((i + 1) * 1000).min(script.len());
        if s >= script.len() {
            break;
        }
        let path = format!("part_{}.txt", i);
        fs::write(&path, &script[s..e]).unwrap();
        println!("part_{}.txt: {}..{} ({} chars)", i, s, e, e - s);
    }
}