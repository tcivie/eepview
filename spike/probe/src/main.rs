#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, thread, time::Duration};

use tauri::{
    AppHandle, Manager, Runtime, Url, WebviewUrl, WebviewWindowBuilder, webview::PlatformWebview,
};

const WEBRTC_OFF_SCRIPT: &str = r"
for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel',
                    'RTCSessionDescription', 'RTCIceCandidate']) {
  try { Object.defineProperty(window, name, { value: undefined, writable: false, configurable: false }); }
  catch (e) {}
}
";

const WIN_DEFAULT_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

struct Probe {
    proxy: Url,
    page: Url,
    harden: bool,
    js: bool,
    win_args: String,
    seconds: u64,
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse_url(key: &str, default: &str) -> Url {
    Url::parse(&env_or(key, default)).unwrap_or_else(|e| panic!("{key} is not a URL: {e}"))
}

impl Probe {
    fn from_env() -> Self {
        Self {
            proxy: parse_url("PROBE_PROXY", "http://127.0.0.1:18080"),
            page: parse_url("PROBE_PAGE", "http://probe.i2p/test.html?mode=manual"),
            harden: env_or("PROBE_HARDEN", "0") == "1",
            js: env_or("PROBE_JS", "on") == "on",
            win_args: env_or("PROBE_WIN_ARGS", "none"),
            seconds: env_or("PROBE_SECONDS", "15").parse().unwrap_or(15),
        }
    }
}

fn hardened_windows_args(proxy: &Url) -> String {
    let host = proxy.host_str().unwrap_or("127.0.0.1");
    let port = proxy.port().unwrap_or(80);
    format!(
        "{WIN_DEFAULT_ARGS} --proxy-server=http://{host}:{port} --proxy-bypass-list=<-loopback> \
         --force-webrtc-ip-handling-policy=disable_non_proxied_udp"
    )
}

fn apply_windows_args<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    probe: &Probe,
) -> WebviewWindowBuilder<'a, R, M> {
    if !cfg!(windows) {
        return builder;
    }
    match probe.win_args.as_str() {
        "hardened" => builder.additional_browser_args(&hardened_windows_args(&probe.proxy)),
        "trap" => builder.additional_browser_args(WIN_DEFAULT_ARGS),
        // S10: no proxy and no wry defaults, so the OS firewall is the only layer left.
        "bare" => builder.additional_browser_args(""),
        _ => builder,
    }
}

/// Turns off WebRTC and media capture in the engine itself. Only `WebKitGTK` exposes this.
#[cfg(target_os = "linux")]
fn harden_engine(webview: &PlatformWebview, harden: bool) {
    use webkit2gtk::{SettingsExt, WebViewExt};
    if !harden {
        return;
    }
    if let Some(settings) = webview.inner().settings() {
        settings.set_enable_webrtc(false);
        settings.set_enable_media_stream(false);
    }
}

/// `WKWebView` and `WebView2` have no engine switch for WebRTC; the init script is the only layer.
#[cfg(not(target_os = "linux"))]
fn harden_engine(_webview: &PlatformWebview, _harden: bool) {}

fn build_window<R: Runtime>(app: &AppHandle<R>, probe: &Probe) -> tauri::Result<()> {
    let mut builder =
        WebviewWindowBuilder::new(app, "probe", WebviewUrl::External(probe.page.clone()))
            .title("eepview probe")
            .proxy_url(probe.proxy.clone())
            .incognito(true);
    if !probe.js {
        builder = builder.disable_javascript();
    }
    if probe.harden {
        builder = builder.initialization_script_for_all_frames(WEBRTC_OFF_SCRIPT);
    }
    let harden = probe.harden;
    apply_windows_args(builder, probe)
        .build()?
        .with_webview(move |webview| harden_engine(&webview, harden))
}

fn exit_after<R: Runtime>(app: AppHandle<R>, seconds: u64) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(seconds));
        app.exit(0);
    });
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let probe = Probe::from_env();
            build_window(app.handle(), &probe)?;
            exit_after(app.handle().clone(), probe.seconds);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the probe");
}
