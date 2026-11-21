//! Best-effort desktop notifications. Failures are ignored: a missing
//! notification daemon must never break a capture.

pub fn info(summary: &str, body: &str) {
    show(summary, body);
}

pub fn error(summary: &str, body: &str) {
    show(summary, body);
}

fn show(summary: &str, body: &str) {
    let mut n = notify_rust::Notification::new();
    n.summary(summary).body(body).appname("FluxShot");
    #[cfg(target_os = "linux")]
    n.icon("camera-photo")
        .timeout(notify_rust::Timeout::Milliseconds(4000));
    if let Err(e) = n.show()
        && std::env::var_os("FLUXSHOT_TRACE").is_some()
    {
        eprintln!("[trace] notification failed: {e}");
    }
}
