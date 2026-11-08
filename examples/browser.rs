/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use glib::info;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box, Entry, Orientation, gio, glib};
use servo_gtk::{LoadEvent, WebView};
use servo_gtk::{BoaRuntime, UserContentManager, UserScript};
use std::cell::RefCell;
use std::rc::Rc;
use std::ptr;
use url::Url;

// Servo's Windows WGL backend uses these exported symbols to select the
// appropriate GPU. Defining them at the executable crate root also prevents
// surfman from writing its warning to stdout, which is reserved for the
// runner's binary IPC stream.
surfman::declare_surfman!();

#[cfg(windows)]
const EPOXY_LIBRARY: &str = "libepoxy-0.dll";

#[cfg(unix)]
const EPOXY_LIBRARY: &str = "libepoxy.so.0";

const G_LOG_DOMAIN: &str = "ServoGtkBrowser";

const LOGGER: glib::GlibLogger = glib::GlibLogger::new(
    glib::GlibLoggerFormat::Plain,
    glib::GlibLoggerDomain::CrateTarget,
);

const SAMPLE_HTML: &str = "<!DOCTYPE html>\
    <html><head><meta charset=\"utf-8\"><title>Inline HTML</title></head>\
    <body style=\"font-family: sans-serif; padding: 2rem;\">\
    <h1>Hello from load_html()</h1>\
    <p>This document was loaded from an in-memory string, \
    not fetched over the network.</p>\
    <p>\
    <button onclick=\"alert('This is an alert dialog.')\">alert()</button>\
    <button onclick=\"document.getElementById('o').textContent = \
    confirm('Do you confirm?')\">confirm()</button>\
    <button onclick=\"document.getElementById('o').textContent = \
    prompt('Type something:', 'default text')\">prompt()</button>\
    </p>\
    <p><input type=\"file\" onchange=\"document.getElementById('o').textContent = \
    this.value\"> (file chooser)</p>\
    <p><a href=\"https://example.org\" target=\"_blank\">Open example.org in a new window</a></p>\
    <p>Result: <span id=\"o\"></span></p>\
    </body></html>";

fn normalize_url(text: &str) -> String {
    let text = text.trim();
    if Url::parse(text).is_ok() {
        text.to_string()
    } else {
        format!("https://{text}")
    }
}

fn main() -> glib::ExitCode {
    // If this process was spawned as the Servo runner subprocess, hand off to
    // it immediately. This never returns when running as the runner.
    servo_gtk::run_as_runner_if_requested();

    log::set_logger(&LOGGER).expect("logger already set");
    log::set_max_level(log::LevelFilter::Debug);

    info!("Starting ServoGtk example app");

    let library = unsafe { libloading::Library::new(EPOXY_LIBRARY) }.unwrap();
    epoxy::load_with(|name| {
        unsafe { library.get::<_>(name.as_bytes()) }
            .map(|symbol| *symbol)
            .unwrap_or(ptr::null())
    });

    let app = Application::builder()
        .application_id("com.example.ServoGtk")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let home_url = std::env::var("SERVO_GTK_HOME_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8000/deno-script.html".to_string());
    app.connect_activate(move |app| {
        open_browser_window(app, &home_url);
    });

    app.run()
}

/// Build and present a browser window hosting a [`WebView`] loaded with `url`.
///
/// Used both for the initial window and, via the `create-web-view` signal, for
/// popups requested by page content (`window.open` / `target=_blank`).
fn open_browser_window(app: &Application, url: &str) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Servo GTK Browser")
        .default_width(1024)
        .default_height(768)
        .build();

    let vbox = Box::new(Orientation::Vertical, 5);
    let hbox = Box::new(Orientation::Horizontal, 5);

    let url_entry = Entry::builder()
        .placeholder_text("Enter URL...")
        .text(url)
        .hexpand(true)
        .build();

    let back_button = gtk::Button::from_icon_name("go-previous");
    back_button.set_tooltip_text(Some("Go Back"));

    let forward_button = gtk::Button::from_icon_name("go-next");
    forward_button.set_tooltip_text(Some("Go Forward"));

    let reload_button = gtk::Button::from_icon_name("view-refresh");
    reload_button.set_tooltip_text(Some("Reload"));

    let zoom_out_button = gtk::Button::from_icon_name("zoom-out-symbolic");
    zoom_out_button.set_tooltip_text(Some("Zoom out"));

    let zoom_reset_button = gtk::Button::from_icon_name("zoom-original-symbolic");
    zoom_reset_button.set_tooltip_text(Some("Reset zoom"));

    let zoom_in_button = gtk::Button::from_icon_name("zoom-in-symbolic");
    zoom_in_button.set_tooltip_text(Some("Zoom in"));

    let html_button = gtk::Button::from_icon_name("text-x-generic-symbolic");
    html_button.set_tooltip_text(Some("Load a sample inline HTML document"));

    let spinner = gtk::Spinner::new();
    spinner.set_tooltip_text(Some("Loading"));

    let user_content_manager = UserContentManager::new();
    user_content_manager.register_script_message_handler("denoScript");
    user_content_manager.add_script(&UserScript::new(
        r#"(function() {
          function scan() {
            document.querySelectorAll('script[type="application/deno"]').forEach(function(node) {
              if (node.dataset.servoBoaHandled) return;
              node.dataset.servoBoaHandled = "true";
              window.servoGtk.messageHandlers.denoScript.postMessage(node.textContent || "");
            });
          }
          if (document.readyState === "loading") {
            document.addEventListener("DOMContentLoaded", scan, { once: true });
          } else {
            scan();
          }
        })();"#,
    ));

    let web_view = WebView::with_user_content_manager(&user_content_manager);
    web_view.set_hexpand(true);
    web_view.set_vexpand(true);

    let boa = Rc::new(RefCell::new(
        BoaRuntime::with_read_only_root(std::env::current_dir().unwrap_or_default())
            .expect("current directory must be readable"),
    ));
    let web_view_for_deno = web_view.clone();
    user_content_manager.connect_script_message_received(move |_, name, body| {
        if name != "denoScript" {
            return;
        }
        let source = serde_json::from_str::<String>(body).unwrap_or_default();
        info!("Evaluating DenoScript ({} bytes)", source.len());
        let result = boa
            .borrow_mut()
            .eval(&source)
            .map(|value| format!("OK: {value}"))
            .unwrap_or_else(|error| format!("ERROR: {error}"));
        info!("DenoScript result: {result}");
        let result_literal = serde_json::to_string(&result).unwrap();
        web_view_for_deno.evaluate_javascript(&format!(
            "(function() {{ var e = document.getElementById('deno-script'); if (e) {{ e.textContent = {}; e.className = {}; }} }})();",
            result_literal, "'ok'"
        ));
    });

    let web_view_clone = web_view.clone();
    url_entry.connect_activate(move |entry| {
        let url = normalize_url(&entry.text());
        web_view_clone.load_url(&url);
    });

    let web_view_clone = web_view.clone();
    reload_button.connect_clicked(move |_| {
        web_view_clone.reload();
    });

    let web_view_clone = web_view.clone();
    back_button.connect_clicked(move |_| {
        web_view_clone.go_back();
    });

    let web_view_clone = web_view.clone();
    forward_button.connect_clicked(move |_| {
        web_view_clone.go_forward();
    });

    // Page zoom: step by a multiplicative factor, or reset to 1.0. The
    // `zoom-level` property is clamped to [0.1, 10.0] by the widget.
    let web_view_clone = web_view.clone();
    zoom_out_button.connect_clicked(move |_| {
        web_view_clone.set_zoom_level(web_view_clone.zoom_level() / 1.2);
    });

    let web_view_clone = web_view.clone();
    zoom_reset_button.connect_clicked(move |_| {
        web_view_clone.set_zoom_level(1.0);
    });

    let web_view_clone = web_view.clone();
    zoom_in_button.connect_clicked(move |_| {
        web_view_clone.set_zoom_level(web_view_clone.zoom_level() * 1.2);
    });

    // Demonstrate loading an in-memory HTML document that also exercises the
    // script dialogs (alert/confirm/prompt), a file chooser and a popup link.
    let web_view_clone = web_view.clone();
    html_button.connect_clicked(move |_| {
        web_view_clone.load_html(SAMPLE_HTML, None);
    });

    // Keep the URL entry in sync with the actual page URI via `notify::uri`.
    // This is how a redirect would be observed.
    let url_entry_clone = url_entry.clone();
    web_view.connect_uri_notify(move |web_view| {
        if let Some(uri) = web_view.uri() {
            url_entry_clone.set_text(&uri);
        }
    });

    // Reflect the page title in the window title via `notify::title`.
    let window_clone = window.clone();
    web_view.connect_title_notify(move |web_view| match web_view.title() {
        Some(title) if !title.is_empty() => {
            window_clone.set_title(Some(&format!("{title} — Servo GTK Browser")));
        }
        _ => window_clone.set_title(Some("Servo GTK Browser")),
    });

    // Show a spinner while a load is in progress via `load-changed`.
    let spinner_clone = spinner.clone();
    web_view.connect_load_changed(move |_web_view, event| match event {
        LoadEvent::Started => spinner_clone.start(),
        LoadEvent::Finished => spinner_clone.stop(),
    });

    // Popups (window.open / target=_blank): open a new browser window hosting
    // a fresh WebView for the requested URL.
    let app_clone = app.clone();
    web_view.connect_create_web_view(move |_web_view, url| {
        info!("Opening popup window for {url}");
        open_browser_window(&app_clone, url);
    });

    hbox.append(&back_button);
    hbox.append(&forward_button);
    hbox.append(&reload_button);
    hbox.append(&url_entry);
    hbox.append(&zoom_out_button);
    hbox.append(&zoom_reset_button);
    hbox.append(&zoom_in_button);
    hbox.append(&html_button);
    hbox.append(&spinner);
    vbox.append(&hbox);
    vbox.append(&web_view);

    window.set_child(Some(&vbox));
    window.present();

    web_view.load_url(url);
}
