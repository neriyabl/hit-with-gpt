use std::path::Path;

use hit_with_gpt::watcher::send_change_to_server;
use serial_test::serial;
use httpmock::Method::POST;
use httpmock::MockServer;


#[test]
#[serial]
fn reports_change_to_server() {
    let server = MockServer::start();
    let partial = serde_json::json!({"hash": "abcd", "path": "foo.txt"}).to_string();
    let mock = server.mock(move |when, then| {
        when.method(POST)
            .path("/changes")
            .header("content-type", "application/json")
            .json_body_partial(partial);
        then.status(200);
    });

    unsafe { std::env::set_var("HIT_SERVER_URL", server.url("")); }

    send_change_to_server("abcd", Path::new("foo.txt")).unwrap();
    mock.assert();
    assert!(mock.hits() >= 1);
}

#[test]
#[serial]
fn error_when_unreachable() {
    unsafe { std::env::set_var("HIT_SERVER_URL", "http://127.0.0.1:59999"); }
    let err = send_change_to_server("abcd", Path::new("foo.txt"));
    assert!(err.is_err());
}
