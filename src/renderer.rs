use std::time::SystemTime;

use actix_web::http::StatusCode;
use chrono::{DateTime, Utc};
use chrono_humanize::Humanize;
use clap::{crate_name, crate_version, ValueEnum};
use fast_qr::{convert::svg::SvgBuilder, qr::QRCodeError, QRBuilder};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use strum::{Display, IntoEnumIterator};

use crate::auth::CurrentUser;
use crate::consts;
use crate::listing::{Breadcrumb, Entry, QueryParameters, SortingMethod, SortingOrder};
use crate::{archive::ArchiveMethod, MiniserveConfig};

#[allow(clippy::too_many_arguments)]
/// Renders the file listing
pub fn page(
    entries: Vec<Entry>,
    readme: Option<(String, String)>,
    abs_url: impl AsRef<str>,
    is_root: bool,
    query_params: QueryParameters,
    breadcrumbs: &[Breadcrumb],
    encoded_dir: &str,
    conf: &MiniserveConfig,
    current_user: Option<&CurrentUser>,
) -> Markup {
    // If query_params.raw is true, we want render a minimal directory listing
    if query_params.raw.is_some() && query_params.raw.unwrap() {
        return raw(entries, is_root);
    }

    let upload_route = format!("{}/upload", &conf.route_prefix);
    let (sort_method, sort_order) = (query_params.sort, query_params.order);

    let upload_action = build_upload_action(&upload_route, encoded_dir, sort_method, sort_order);
    let mkdir_action = build_mkdir_action(&upload_route, encoded_dir);

    let title_path = breadcrumbs_to_path_string(breadcrumbs);

    let upload_allowed = conf.allowed_upload_dir.is_empty()
        || conf
            .allowed_upload_dir
            .iter()
            .any(|x| encoded_dir.starts_with(&format!("/{}", x)));

    html! {
        (DOCTYPE)
        html {
            (page_header(&title_path, conf.file_upload, &conf.favicon_route, &conf.css_route))

            body #drop-container
                .(format!("default_theme_{}", conf.default_color_scheme))
                .(format!("default_theme_dark_{}", conf.default_color_scheme_dark)) {

                (PreEscaped(r#"
                    <script>
                        // read theme from local storage and apply it to body
                        const body = document.body;
                        var theme = localStorage.getItem('theme');

                        if (theme != null && theme != 'default') {
                            body.classList.add('theme_' + theme);
                        }

                        // updates the color scheme by replacing the appropriate class
                        // on body and saving the new theme to local storage
                        function updateColorScheme(name) {
                            body.classList.remove.apply(body.classList, Array.from(body.classList).filter(v=>v.startsWith("theme_")));

                            if (name != "default") {
                                body.classList.add('theme_' + name);
                            }

                            localStorage.setItem('theme', name);
                        }
                    </script>
                    "#))

                div.toolbar_box_group {
                    @if conf.file_upload {
                        div.form {
                            div.form_title {
                                h1 { "Drop your file here to upload it" }
                            }
                        }
                    }

                    @if conf.mkdir_enabled {
                        div.form {
                            div.form_title {
                                h1 { "Create a new directory" }
                            }
                        }
                    }
                }
                nav {
                    (qr_spoiler(conf.show_qrcode, abs_url))
                    (color_scheme_selector(conf.hide_theme_selector))
                }
                div.container {
                    span #top { }
                    h1.title dir="ltr" {
                        @for el in breadcrumbs {
                            @if el.link == "." {
                                // wrapped in span so the text doesn't shift slightly when it turns into a link
                                span { bdi { (el.name) } }
                            } @else {
                                a href=(parametrized_link(&el.link, sort_method, sort_order, false)) {
                                    bdi { (el.name) }
                                }
                            }
                            "/"
                        }
                    }
                    div.toolbar {
                        @if conf.tar_enabled || conf.tar_gz_enabled || conf.zip_enabled {
                            div.download {
                                @for archive_method in ArchiveMethod::iter() {
                                    @if archive_method.is_enabled(conf.tar_enabled, conf.tar_gz_enabled, conf.zip_enabled) {
                                        (archive_button(archive_method, sort_method, sort_order))
                                    }
                                }
                            }
                        }
                        div.toolbar_box_group {
                            @if conf.file_upload && upload_allowed {
                                div.toolbar_box {
                                    form id="file_submit" action=(upload_action) method="POST" enctype="multipart/form-data" {
                                        p { "Select a file to upload or drag it anywhere into the window" }
                                        div {
                                            @match &conf.uploadable_media_type {
                                                Some(accept) => {input #file-input accept=(accept) type="file" name="file_to_upload" required="" multiple {}},
                                                None => {input #file-input type="file" name="file_to_upload" required="" multiple {}}
                                            }
                                            button type="submit" { "Upload file" }
                                        }
                                    }
                                }
                            }
                            @if conf.mkdir_enabled {
                                div.toolbar_box {
                                    form id="mkdir" action=(mkdir_action) method="POST" enctype="multipart/form-data" {
                                        p { "Specify a directory name to create" }
                                        div.toolbar_box {
                                            input type="text" name="mkdir" required="" placeholder="Directory name" {}
                                            button type="submit" { "Create directory" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    table {
                        thead {
                            th.name { (build_link("name", "Name", sort_method, sort_order)) }
                            th.size { (build_link("size", "Size", sort_method, sort_order)) }
                            th.date { (build_link("date", "Last modification", sort_method, sort_order)) }
                        }
                        tbody {
                            @if !is_root {
                                tr {
                                    td colspan="3" {
                                        p {
                                            span.root-chevron { (chevron_left()) }
                                            a.root href=(parametrized_link("../", sort_method, sort_order, false)) {
                                                "Parent directory"
                                            }
                                        }
                                    }
                                }
                            }
                            @for entry in entries {
                                (entry_row(entry, sort_method, sort_order, false))
                            }
                        }
                    }
                    @if let Some(readme) = readme {
                        div id="readme" {
                            h3 id="readme-filename" { (readme.0) }
                            div id="readme-contents" {
                                (PreEscaped (readme.1))
                            };
                        }
                    }
                    a.back href="#top" {
                        (arrow_up())
                    }
                    div.footer {
                        @if conf.show_wget_footer {
                            (wget_footer(&title_path, current_user))
                        }
                        @if !conf.hide_version_footer {
                            (version_footer())
                        }
                    }
                }
            }
        }
    }
}

/// Renders the file listing
pub fn raw(entries: Vec<Entry>, is_root: bool) -> Markup {
    html! {
        (DOCTYPE)
        html {
            body {
                table {
                    thead {
                        th.name { "Name" }
                        th.size { "Size" }
                        th.date { "Last modification" }
                    }
                    tbody {
                        @if !is_root {
                            tr {
                                td colspan="3" {
                                    p {
                                        a.root href=(parametrized_link("../", None, None, true)) {
                                            ".."
                                        }
                                    }
                                }
                            }
                        }
                        @for entry in entries {
                            (entry_row(entry, None, None, true))
                        }
                    }
                }
            }
        }
    }
}

/// Renders the QR code SVG
fn qr_code_svg(url: impl AsRef<str>, margin: usize) -> Result<String, QRCodeError> {
    let qr = QRBuilder::new(url.as_ref().into())
        .ecl(consts::QR_EC_LEVEL)
        .build()?;
    let svg = SvgBuilder::default().margin(margin).to_str(&qr);

    Ok(svg)
}

/// Build a path string from a list of breadcrumbs.
fn breadcrumbs_to_path_string(breadcrumbs: &[Breadcrumb]) -> String {
    breadcrumbs
        .iter()
        .map(|el| el.name.clone())
        .collect::<Vec<_>>()
        .join("/")
}

// Partial: version footer
fn version_footer() -> Markup {
    html! {
       div.version {
           (format!("{}/{}", crate_name!(), crate_version!()))
       }
    }
}

fn wget_footer(title_path: &str, current_user: Option<&CurrentUser>) -> Markup {
    let count = {
        let count_slashes = title_path.matches('/').count();
        if count_slashes > 0 {
            count_slashes - 1
        } else {
            0
        }
    };

    let user_params = if let Some(user) = current_user {
        format!(" --ask-password --user {}", user.name)
    } else {
        "".to_string()
    };

    html! {
        div.downloadDirectory {
            p { "Download folder:" }
            div.cmd { (format!("wget -r -c -nH -np --cut-dirs={} -R \"index.html*\"{} \"http://{}/?raw=true\"", count, user_params, title_path)) }
        }
    }
}

/// Build the action of the upload form
fn build_upload_action(
    upload_route: &str,
    encoded_dir: &str,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
) -> String {
    let mut upload_action = format!("{}?path={}", upload_route, encoded_dir);
    if let Some(sorting_method) = sort_method {
        upload_action = format!("{}&sort={}", upload_action, &sorting_method);
    }
    if let Some(sorting_order) = sort_order {
        upload_action = format!("{}&order={}", upload_action, &sorting_order);
    }

    upload_action
}

/// Build the action of the mkdir form
fn build_mkdir_action(mkdir_route: &str, encoded_dir: &str) -> String {
    format!("{}?path={}", mkdir_route, encoded_dir)
}

const THEME_PICKER_CHOICES: &[(&str, &str)] = &[
    ("Default (light/dark)", "default"),
    ("Squirrel (light)", "squirrel"),
    ("Arch Linux (dark)", "archlinux"),
    ("Zenburn (dark)", "zenburn"),
    ("Monokai (dark)", "monokai"),
];

#[derive(Debug, Clone, ValueEnum, Display)]
pub enum ThemeSlug {
    #[strum(serialize = "squirrel")]
    Squirrel,
    #[strum(serialize = "archlinux")]
    Archlinux,
    #[strum(serialize = "zenburn")]
    Zenburn,
    #[strum(serialize = "monokai")]
    Monokai,
}

/// Partial: qr code spoiler
fn qr_spoiler(show_qrcode: bool, content: impl AsRef<str>) -> Markup {
    html! {
        @if show_qrcode {
            div {
                p {
                    "QR code"
                }
                div.qrcode #qrcode title=(PreEscaped(content.as_ref())) {
                    @match qr_code_svg(content, consts::SVG_QR_MARGIN) {
                        Ok(svg) => (PreEscaped(svg)),
                        Err(err) => (format!("QR generation error: {:?}", err)),
                    }
                }
            }
        }
    }
}

/// Partial: color scheme selector
fn color_scheme_selector(hide_theme_selector: bool) -> Markup {
    html! {
        @if !hide_theme_selector {
            div {
                p {
                    "Change theme..."
                }
                ul.theme {
                    @for color_scheme in THEME_PICKER_CHOICES {
                        li.(format!("theme_{}", color_scheme.1)) {
                            (color_scheme_link(color_scheme))
                        }
                    }
                }
            }
        }
    }
}

// /// Partial: color scheme link
fn color_scheme_link(color_scheme: &(&str, &str)) -> Markup {
    let title = format!("Switch to {} theme", color_scheme.0);

    html! {
        a href=(format!("javascript:updateColorScheme(\"{}\")", color_scheme.1)) title=(title) {
            (color_scheme.0)
        }
    }
}

/// Partial: archive button
fn archive_button(
    archive_method: ArchiveMethod,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
) -> Markup {
    let link = if sort_method.is_none() && sort_order.is_none() {
        format!("?download={}", archive_method)
    } else {
        format!(
            "{}&download={}",
            parametrized_link("", sort_method, sort_order, false),
            archive_method
        )
    };

    let text = format!("Download .{}", archive_method.extension());

    html! {
        a href=(link) {
            (text)
        }
    }
}

/// Ensure that there's always a trailing slash behind the `link`.
fn make_link_with_trailing_slash(link: &str) -> String {
    if link.ends_with('/') {
        link.to_string()
    } else {
        format!("{}/", link)
    }
}

/// If they are set, adds query parameters to links to keep them across pages
fn parametrized_link(
    link: &str,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    raw: bool,
) -> String {
    if raw {
        return format!("{}?raw=true", make_link_with_trailing_slash(link));
    }

    if let Some(method) = sort_method {
        if let Some(order) = sort_order {
            let parametrized_link = format!(
                "{}?sort={}&order={}",
                make_link_with_trailing_slash(link),
                method,
                order,
            );

            return parametrized_link;
        }
    }

    make_link_with_trailing_slash(link)
}

/// Partial: table header link
fn build_link(
    name: &str,
    title: &str,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
) -> Markup {
    let mut link = format!("?sort={}&order=asc", name);
    let mut help = format!("Sort by {} in ascending order", name);
    let mut chevron = chevron_up();
    let mut class = "";

    if let Some(method) = sort_method {
        if method.to_string() == name {
            class = "active";
            if let Some(order) = sort_order {
                if order.to_string() == "asc" {
                    link = format!("?sort={}&order=desc", name);
                    help = format!("Sort by {} in descending order", name);
                    chevron = chevron_down();
                }
            }
        }
    };

    html! {
        span class=(class) {
            span.chevron { (chevron) }
            a href=(link) title=(help) { (title) }
        }
    }
}

/// Partial: row for an entry
fn entry_row(
    entry: Entry,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    raw: bool,
) -> Markup {
    html! {
        tr {
            td {
                p {
                    @if entry.is_dir() {
                        @if let Some(symlink_dest) = entry.symlink_info {
                            a.symlink href=(parametrized_link(&entry.link, sort_method, sort_order, raw)) {
                                (entry.name) "/"
                                span.symlink-symbol { }
                                a.directory {(symlink_dest) "/"}
                            }
                        }@else {
                            a.directory href=(parametrized_link(&entry.link, sort_method, sort_order, raw)) {
                                (entry.name) "/"
                            }
                        }
                    } @else if entry.is_file() {
                        @if let Some(symlink_dest) = entry.symlink_info {
                            a.symlink href=(&entry.link) {
                                (entry.name)
                                span.symlink-symbol { }
                                a.file {(symlink_dest)}
                            }
                        }@else {
                            a.file href=(&entry.link) {
                                (entry.name)
                            }
                        }

                        @if !raw {
                            span.mobile-info.size {
                                (maud::display(&entry.size))
                            }
                            // @if let size = entry.size {
                            //     span.mobile-info.size {
                            //         (maud::display(size))
                            //     }
                            // }
                        }
                    }
                }
            }
            td.size-cell {
                (maud::display(&entry.size))
                // @if let size = entry.size {
                //     (maud::display(size))
                // }
            }
            td.date-cell {
                @if let Some(modification_date) = convert_to_utc(entry.last_modification_date) {
                    span {
                        (modification_date.0) " "
                        span.at { " at " }
                        (modification_date.1) " "
                    }
                }
                @if let Some(modification_timer) = humanize_systemtime(entry.last_modification_date) {
                    span.history {
                        (modification_timer)
                    }
                }
            }
        }
    }
}

/// Partial: up arrow
fn arrow_up() -> Markup {
    PreEscaped("⇪".to_string())
}

/// Partial: chevron left
fn chevron_left() -> Markup {
    PreEscaped("◂".to_string())
}

/// Partial: chevron up
fn chevron_up() -> Markup {
    PreEscaped("▴".to_string())
}

/// Partial: chevron up
fn chevron_down() -> Markup {
    PreEscaped("▾".to_string())
}

/// Partial: page header
fn page_header(title: &str, file_upload: bool, favicon_route: &str, css_route: &str) -> Markup {
    html! {
        head {
            meta charset="utf-8";
            meta http-equiv="X-UA-Compatible" content="IE=edge";
            meta name="viewport" content="width=device-width, initial-scale=1";

            link rel="icon" type="image/svg+xml" href={ (favicon_route) };
            link rel="stylesheet" href={ (css_route) };

            title { (title) }

            @if file_upload {
                (PreEscaped(r#"
                <script>
                    window.onload = function() {
                        const dropContainer = document.querySelector('#drop-container');
                        const dragForm = document.querySelector('.drag-form');
                        const fileInput = document.querySelector('#file-input');
                        const collection = [];

                        dropContainer.ondragover = function(e) {
                            e.preventDefault();
                        }

                        dropContainer.ondragenter = function(e) {
                            e.preventDefault();
                            if (collection.length === 0) {
                                dragForm.style.display = 'initial';
                            }
                            collection.push(e.target);
                        };

                        dropContainer.ondragleave = function(e) {
                            e.preventDefault();
                            collection.splice(collection.indexOf(e.target), 1);
                            if (collection.length === 0) {
                                dragForm.style.display = 'none';
                            }
                        };

                        dropContainer.ondrop = function(e) {
                            e.preventDefault();
                            fileInput.files = e.dataTransfer.files;
                            file_submit.submit();
                            dragForm.style.display = 'none';
                        };
                    }
                </script>
                "#))
            }
        }
    }
}

/// Converts a SystemTime object to a strings tuple (date, time)
/// Date is formatted as %e %b, e.g. Jul 12
/// Time is formatted as %R, e.g. 22:34
fn convert_to_utc(src_time: Option<SystemTime>) -> Option<(String, String)> {
    src_time.map(DateTime::<Utc>::from).map(|date_time| {
        (
            date_time.format("%b %e").to_string(),
            date_time.format("%R").to_string(),
        )
    })
}

/// Converts a SystemTime to a string readable by a human,
/// and gives a rough approximation of the elapsed time since
fn humanize_systemtime(time: Option<SystemTime>) -> Option<String> {
    time.map(|time| time.humanize())
}

/// Renders an error on the webpage
pub fn render_error(
    error_description: &str,
    error_code: StatusCode,
    conf: &MiniserveConfig,
    return_address: &str,
) -> Markup {
    html! {
        (DOCTYPE)
        html {
            (page_header(&error_code.to_string(), false, &conf.favicon_route, &conf.css_route))

            body.(format!("default_theme_{}", conf.default_color_scheme))
                .(format!("default_theme_dark_{}", conf.default_color_scheme_dark)) {

                (PreEscaped(r#"
                    <script>
                        // read theme from local storage and apply it to body
                        var theme = localStorage.getItem('theme');
                        if (theme != null && theme != 'default') {
                            document.body.classList.add('theme_' + theme);
                        }
                    </script>
                    "#))

                div.error {
                    p { (error_code.to_string()) }
                    @for error in error_description.lines() {
                        p { (error) }
                    }
                    // WARN don't expose random route!
                    @if conf.route_prefix.is_empty() {
                        div.error-nav {
                            a.error-back href=(return_address) {
                                "Go back to file listing"
                            }
                        }
                    }
                    @if !conf.hide_version_footer {
                        p.footer {
                            (version_footer())
                        }

                    }
                }
            }
        }
    }
}
