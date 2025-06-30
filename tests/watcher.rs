use std::path::Path;
use hit_with_gpt::watcher::send_change_to_server;
use httpmock::prelude::*;

#[test]
fn send_change_posts_json() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/changes")
            .json_body_partial(r#"{ "hash": "abc123", "path": "src/lib.rs" }"#);
        then.status(200);
    });

    unsafe { std::env::set_var("HIT_SERVER_URL", server.url("")); }
    send_change_to_server("abc123", Path::new("src/lib.rs")).unwrap();
    mock.assert();
    unsafe { std::env::remove_var("HIT_SERVER_URL"); }
}

#[test]
fn send_change_handles_unreachable() {
    unsafe { std::env::set_var("HIT_SERVER_URL", "http://127.0.0.1:59999"); }
    let result = send_change_to_server("abc", Path::new("foo.txt"));
    assert!(result.is_err());
    unsafe { std::env::remove_var("HIT_SERVER_URL"); }
}
