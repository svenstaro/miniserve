use chrono::{DateTime, Duration, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use std::time::SystemTime;
use strum::IntoEnumIterator;

use crate::archive::CompressionMethod;
use crate::listing::{Entry, SortingMethod, SortingOrder};
use crate::themes::ColorScheme;

/// Renders the file listing
pub fn page(
    serve_path: &str,
    entries: Vec<Entry>,
    is_root: bool,
    page_parent: Option<String>,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    default_color_scheme: ColorScheme,
    color_scheme: ColorScheme,
    file_upload: bool,
    upload_route: &str,
    current_dir: &str,
) -> Markup {
    let upload_action = build_upload_action(
        upload_route,
        current_dir,
        sort_method,
        sort_order,
        color_scheme,
        default_color_scheme,
    );

    html! {
        (page_header(serve_path, color_scheme, file_upload, false))
        body#drop-container {
            @if file_upload {
                div.drag-form {
                    div.drag-title {
                        h1 { "Drop your file here to upload it" }
                    }
                }
            }
            (color_scheme_selector(sort_method, sort_order, color_scheme, default_color_scheme, serve_path))
            div.container {
                span#top { }
                h1.title { "Index of " (serve_path) }
                div.toolbar {
                    div.download {
                        @for compression_method in CompressionMethod::iter() {
                            (archive_button(compression_method, sort_method, sort_order, color_scheme, default_color_scheme))
                        }
                    }
                    @if file_upload {
                        div.upload {
                            form id="file_submit" action=(upload_action) method="POST" enctype="multipart/form-data" {
                                p { "Select a file to upload or drag it anywhere into the window" }
                                div {
                                    input#file-input type="file" name="file_to_upload" required="" {}
                                    button type="submit" { "Upload file" }
                                }
                            }
                        }
                    }
                }
                table {
                    thead {
                        th { (build_link("name", "Name", sort_method, sort_order, color_scheme, default_color_scheme)) }
                        th { (build_link("size", "Size", sort_method, sort_order, color_scheme, default_color_scheme)) }
                        th { (build_link("date", "Last modification", sort_method, sort_order, color_scheme, default_color_scheme)) }
                    }
                    tbody {
                        @if !is_root {
                            @if let Some(parent) = page_parent {
                                tr {
                                    td colspan="3" {
                                        span.root-chevron { (chevron_left()) }
                                        a.root href=(parametrized_link(&parent, sort_method, sort_order, color_scheme, default_color_scheme)) {
                                            "Parent directory"
                                        }
                                    }
                                }
                            }
                        }
                        @for entry in entries {
                            (entry_row(entry, sort_method, sort_order, color_scheme, default_color_scheme))
                        }
                    }
                }
                a.back href="#top" {
                    (arrow_up())
                }
            }
        }
    }
}

/// Build the action of the upload form
fn build_upload_action(
    upload_route: &str,
    current_dir: &str,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
) -> String {
    let mut upload_action = format!("{}?path={}", upload_route, current_dir);
    if let Some(sorting_method) = sort_method {
        upload_action = format!("{}&sort={}", upload_action, &sorting_method);
    }
    if let Some(sorting_order) = sort_order {
        upload_action = format!("{}&order={}", upload_action, &sorting_order);
    }
    if color_scheme != default_color_scheme {
        upload_action = format!("{}&theme={}", upload_action, color_scheme.to_slug());
    }

    upload_action
}

/// Partial: color scheme selector
fn color_scheme_selector(
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    active_color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
    serve_path: &str,
) -> Markup {
    html! {
        nav {
            ul {
                li {
                    a.change-theme href="#" title="Change theme" {
                        "Change theme..."
                    }
                    ul {
                        @for color_scheme in ColorScheme::iter() {
                            @if active_color_scheme == color_scheme {
                                li.active {
                                    (color_scheme_link(sort_method, sort_order, color_scheme, default_color_scheme, serve_path))
                                }
                            } @else {
                                li {
                                    (color_scheme_link(sort_method, sort_order, color_scheme, default_color_scheme, serve_path))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Partial: color scheme link
fn color_scheme_link(
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
    serve_path: &str,
) -> Markup {
    let link = parametrized_link(
        serve_path,
        sort_method,
        sort_order,
        color_scheme,
        default_color_scheme,
    );
    let title = format!("Switch to {} theme", color_scheme);

    html! {
        a href=(link) title=(title) {
            (color_scheme)
            " "
            @if color_scheme.is_dark() {
                "(dark)"
            } @else {
                "(light)"
            }
        }
    }
}

/// Partial: archive button
fn archive_button(
    compress_method: CompressionMethod,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
) -> Markup {
    let link =
        if sort_method.is_none() && sort_order.is_none() && color_scheme == default_color_scheme {
            format!("?download={}", compress_method)
        } else {
            format!(
                "{}&download={}",
                parametrized_link(
                    "",
                    sort_method,
                    sort_order,
                    color_scheme,
                    default_color_scheme
                ),
                compress_method
            )
        };

    let text = format!("Download .{}", compress_method.extension());

    html! {
        a href=(link) {
            (text)
        }
    }
}

/// If they are set, adds query parameters to links to keep them across pages
fn parametrized_link(
    link: &str,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
) -> String {
    if let Some(method) = sort_method {
        if let Some(order) = sort_order {
            let parametrized_link = format!("{}?sort={}&order={}", link, method, order);

            if color_scheme != default_color_scheme {
                return format!("{}&theme={}", parametrized_link, color_scheme.to_slug());
            }

            return parametrized_link;
        }
    }

    if color_scheme != default_color_scheme {
        return format!("{}?theme={}", link.to_string(), color_scheme.to_slug());
    }

    link.to_string()
}

/// Partial: table header link
fn build_link(
    name: &str,
    title: &str,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
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

    if color_scheme != default_color_scheme {
        link = format!("{}&theme={}", &link, color_scheme.to_slug());
    }

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
    color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
) -> Markup {
    html! {
        tr {
            td {
                p {
                    @if entry.is_dir() {
                        a.directory href=(parametrized_link(&entry.link, sort_method, sort_order, color_scheme, default_color_scheme)) {
                            (entry.name) "/"
                        }
                    } @else if entry.is_file() {
                        div.file-entry {
                            a.file href=(&entry.link) {
                                (entry.name)
                            }
                            @if let Some(size) = entry.size {
                                span.mobile-info.size {
                                    (size)
                                }
                            }
                        }
                    } @else if entry.is_symlink() {
                        a.symlink href=(parametrized_link(&entry.link, sort_method, sort_order, color_scheme, default_color_scheme)) {
                           (entry.name)  span.symlink-symbol { "⇢" }
                        }
                    }
                }
            }
            td {
                @if let Some(size) = entry.size {
                    (size)
                }
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

/// Partial: CSS
fn css(color_scheme: ColorScheme) -> Markup {
    let theme = color_scheme.get_theme();

    let css = format!("
     html {{
        font-smoothing: antialiased;
        text-rendering: optimizeLegibility;
        width: 100%;
        height: 100%;
    }}
    body {{
        margin: 0;
        font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto,\"Helvetica Neue\", Helvetica, Arial, sans-serif;
        font-weight: 300;
        color: {text_color};
        background: {background};
        position: relative;
        min-height: 100%;
    }}
    .container {{
        padding: 1.5rem 5rem;
    }}
    .title {{
        word-break: break-word;
    }}
    a {{
        text-decoration: none;
    }}
    a.root, a.root:visited, .root-chevron {{
        font-weight: bold;
        color: {root_link_color};
    }}
    a:hover {{
        text-decoration: underline;
    }}
    a.directory, a.directory:visited {{
        font-weight: bold;
        color: {directory_link_color};
    }}
    a.file, a.file:visited, .error-back, .error-back:visited {{
        color: {file_link_color};
    }}
    a.symlink, a.symlink:visited {{
        color: {symlink_link_color};
    }}
    a.directory:hover {{
        color: {directory_link_color};
    }}
    a.file:hover {{
        color: {file_link_color};
    }}
    a.symlink:hover {{
        color: {symlink_link_color};
    }}
    .symlink-symbol {{
        display: inline-block;
        border: 1px solid {symlink_link_color};
        margin-left: 0.5rem;
        border-radius: .2rem;
        padding: 0 0.1rem;
    }}
    nav {{
        padding: 0 5rem;
    }}
    nav ul {{
        text-align: right;
        list-style: none;
        margin: 0;
        padding: 0;
    }}
    nav ul li {{
        display: block;
        transition-duration: 0.5s;
        float: right;
        position: relative;
        padding: 0.5rem 1rem;
        background: {switch_theme_background};
        width: 8rem;
        text-align: center;
    }}
    nav ul li:hover {{
        cursor: pointer;
        text-decoration: none;
        color: {change_theme_link_color}
    }}
    nav ul li a:hover {{
        text-decoration: none;
        color: {change_theme_link_color_hover};
    }}
    nav ul li ul {{
        visibility: hidden;
        opacity: 0;
        position: absolute;
        transition: all 0.5s ease;
        margin-top: 0.5rem;
        left: 0;
        display: none;
        text-align: center;
    }}
    nav ul li:hover > ul,
    nav ul li ul:hover {{
        visibility: visible;
        opacity: 1;
        display: block;
    }}
    nav ul li ul li:first-of-type {{
        border-top: 1px solid {switch_theme_border};
    }}
    nav ul li ul li {{
        clear: both;
        width: 8rem;
        padding-top: 0.5rem;
        padding-bottom: 0.5rem;
    }}
    nav ul li ul li a:hover {{
        text-decoration: underline;
    }}
    nav ul li a, nav ul li ul li a, nav ul li a:visited, nav ul li ul li a:visited {{
        color: {switch_theme_link_color}
    }}
    nav ul li ul li.active a {{
        font-weight: bold;
        color: {switch_theme_active};
    }}
    p {{
        margin: 0;
        padding: 0;
    }}
    h1 {{
        margin-top: 0;
        font-size: 1.5rem;
    }}
    table {{
        margin-top: 2rem;
        width: 100%;
        border: 0;
        table-layout: auto;
        background: {table_background};
    }}
    table thead tr th,
    table tbody tr td {{
        padding: 0.5625rem 0.625rem;
        font-size: 0.875rem;
        color: {table_text_color};
        text-align: left;
        line-height: 1.125rem;
        width: 33.333%;
    }}
    table thead tr th {{
        padding: 0.5rem 0.625rem 0.625rem;
        font-weight: bold;
    }}
    table tbody tr:nth-child(odd) {{
        background: {odd_row_background};
    }}
    table tbody tr:nth-child(even) {{
        background: {even_row_background};
    }}
    table thead {{
        background: {table_header_background};
    }}
    table tbody tr:hover {{
        background: {active_row_color};
    }}
    td.date-cell {{
        display: flex;
        width: calc(100% - 1.25rem);
        justify-content: space-between;
    }}
    .at {{
        color: {at_color};
    }}
    .history {{
        color: {date_text_color};
    }}
    .file-entry {{
        display: flex;
        justify-content: space-between;
    }}
    span.size {{
        border-radius: 1rem;
        background: {size_background_color};
        padding: 0 0.25rem;
        font-size: 0.7rem;
        color: {size_text_color}
    }}
    .mobile-info {{
        display: none;
    }}
    th a, th a:visited, .chevron {{
        color: {table_header_text_color};
    }}
    .chevron, .root-chevron {{
        margin-right: .5rem;
        font-size: 1.2em;
        font-weight: bold;
    }}
    th span.active a, th span.active span {{
        color: {table_header_active_color};
    }}
    .back {{
        position: fixed;
        width: 3.8rem;
        height: 3.8rem;
        align-items: center;
        justify-content: center;
        bottom: 3rem;
        right: 3.75rem;
        background: {back_button_background};
        border-radius: 100%;
        box-shadow: 0 0 8px -4px #888888;
        color: {back_button_link_color};
        display: none;
    }}
    .back:visited {{
        color: {back_button_link_color};
    }}
    .back:hover {{
        color: {back_button_link_color_hover};
        font-weight: bold;
        text-decoration: none;
        background: {back_button_background_hover};
    }}
    .toolbar {{
        display: flex;
        justify-content: space-between;
        flex-wrap: wrap;
    }}
    .download {{
        margin-top: 1rem;
        padding: 0.125rem;
        display: flex;
        flex-direction: row;
        align-items: flex-start;
        flex-wrap: wrap;
    }}
    .download a, .download a:visited {{
        color: {download_button_link_color};
    }}
    .download a {{
        background: {download_button_background};
        padding: 0.5rem;
        border-radius: 0.2rem;
    }}
    .download a:hover {{
        background: {download_button_background_hover};
        color: {download_button_link_color_hover};
    }}
    .download a:not(:last-of-type) {{
        margin-right: 1rem;
    }}
    .upload {{
        margin-top: 1rem;
        display: flex;
        justify-content: flex-end;
    }}
    .upload p {{
        font-size: 0.8rem;
        margin-bottom: 1rem;
        color: {upload_text_color};
    }}
    .upload form {{
        padding: 1rem;
        border: 1px solid {upload_form_border_color};
        background: {upload_form_background};
    }}
    .upload button {{
        background: {upload_button_background};
        padding: 0.5rem;
        border-radius: 0.2rem;
        color: {upload_button_text_color};
        border: none;
    }}
    .upload div {{
        display: flex;
        align-items: baseline;
        justify-content: space-between;
    }}
    .drag-form {{
        display: none;
        background: {drag_background};
        position: absolute;
        border: 0.5rem dashed {drag_border_color};
        width: calc(100% - 1rem);
        height: calc(100% - 1rem);
        text-align: center;
        z-index: 2;
    }}
    .drag-title {{
        position: fixed;
        color: {drag_text_color};
        top: 50%;
        width: 100%;
        text-align: center;
    }}
    .error {{
        margin: 2rem;
    }}
    .error p {{
        margin: 1rem 0;
        font-size: 0.9rem;
        word-break: break-all;
    }}
    .error p:first-of-type {{
        font-size: 1.25rem;
        color: {error_color};
        margin-bottom: 2rem;
    }}
    .error p:nth-of-type(2) {{
        font-weight: bold;
    }}
    .error-nav {{
        margin-top: 4rem;
    }}
    @media (max-width: 760px) {{
        nav {{
            padding: 0 2.5rem;
        }}
        .container {{
            padding: 1.5rem 2.5rem;
        }}
        h1 {{
            font-size: 1.4em;
        }}
        td:not(:nth-child(1)), th:not(:nth-child(1)){{
            display: none;
        }}
        .mobile-info {{
            display: block;
        }}
        .back {{
            display: flex;
        }}
        .back {{
            right: 1.5rem;
        }}
    }}
    @media (max-width: 600px) {{
        h1 {{
            font-size: 1.375em;
        }}
    }}
    @media (max-width: 400px) {{
        nav {{
            padding: 0 0.5rem;
        }}
        .container {{
            padding: 0.5rem;
        }}
        h1 {{
            font-size: 1.375em;
        }}
        .back {{
            right: 1.5rem;
        }}
    }}", background = theme.background,
        text_color = theme.text_color,
        directory_link_color = theme.directory_link_color,
        file_link_color = theme.file_link_color,
        symlink_link_color = theme.symlink_link_color,
        table_background = theme.table_background,
        table_text_color = theme.table_text_color,
        table_header_background = theme.table_header_background,
        table_header_text_color = theme.table_header_text_color,
        table_header_active_color = theme.table_header_active_color,
        active_row_color = theme.active_row_color,
        odd_row_background = theme.odd_row_background,
        even_row_background = theme.even_row_background,
        root_link_color = theme.root_link_color,
        download_button_background = theme.download_button_background,
        download_button_background_hover = theme.download_button_background_hover,
        download_button_link_color = theme.download_button_link_color,
        download_button_link_color_hover = theme.download_button_link_color_hover,
        back_button_background = theme.back_button_background,
        back_button_background_hover = theme.back_button_background_hover,
        back_button_link_color = theme.back_button_link_color,
        back_button_link_color_hover = theme.back_button_link_color_hover,
        date_text_color = theme.date_text_color,
        at_color = theme.at_color,
        switch_theme_background = theme.switch_theme_background,
        switch_theme_link_color = theme.switch_theme_link_color,
        switch_theme_active = theme.switch_theme_active,
        switch_theme_border = theme.switch_theme_border,
        change_theme_link_color = theme.change_theme_link_color,
        change_theme_link_color_hover = theme.change_theme_link_color_hover,
        upload_text_color = theme.upload_text_color,
        upload_form_border_color = theme.upload_form_border_color,
        upload_form_background = theme.upload_form_background,
        upload_button_background = theme.upload_button_background,
        upload_button_text_color = theme.upload_button_text_color,
        drag_background = theme.drag_background,
        drag_border_color = theme.drag_border_color,
        drag_text_color = theme.drag_text_color,
        size_background_color = theme.size_background_color,
        size_text_color = theme.size_text_color,
        error_color = theme.error_color);
    (PreEscaped(css))
}

/// Partial: up arrow
fn arrow_up() -> Markup {
    (PreEscaped("⇪".to_string()))
}

/// Partial: chevron left
fn chevron_left() -> Markup {
    (PreEscaped("◂".to_string()))
}

/// Partial: chevron up
fn chevron_up() -> Markup {
    (PreEscaped("▴".to_string()))
}

/// Partial: chevron up
fn chevron_down() -> Markup {
    (PreEscaped("▾".to_string()))
}

/// Partial: page header
fn page_header(
    serve_path: &str,
    color_scheme: ColorScheme,
    file_upload: bool,
    is_error: bool,
) -> Markup {
    html! {
        (DOCTYPE)
        html {
            meta charset="utf-8";
            meta http-equiv="X-UA-Compatible" content="IE=edge";
            meta name="viewport" content="width=device-width, initial-scale=1";
            @if is_error {
                title { (serve_path) }
            } else {
                title { "Index of " (serve_path) }
            }
            style { (css(color_scheme)) }
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
/// i.e. calculates the duration between now() and the given SystemTime,
/// and gives a rough approximation of the elapsed time since
fn humanize_systemtime(src_time: Option<SystemTime>) -> Option<String> {
    src_time
        .and_then(|std_time| SystemTime::now().duration_since(std_time).ok())
        .and_then(|from_now| Duration::from_std(from_now).ok())
        .map(|duration| HumanTime::from(duration).to_text_en(Accuracy::Rough, Tense::Past))
}

/// Renders an error on the webpage
pub fn render_error(
    error_description: &str,
    return_address: &str,
    sort_method: Option<SortingMethod>,
    sort_order: Option<SortingOrder>,
    color_scheme: ColorScheme,
    default_color_scheme: ColorScheme,
    has_referer: bool,
) -> Markup {
    let link = if has_referer {
        return_address.to_string()
    } else {
        parametrized_link(
            return_address,
            sort_method,
            sort_order,
            color_scheme,
            default_color_scheme,
        )
    };

    html! {
        body {
            (page_header("Error", color_scheme, false, true))
            div.error {
                p { "Error" }
                @for error in error_description.lines() {
                    p { (error) }
                }
                div.error-nav {
                    a.error-back href=(link) {
                        "Go back to file listing"
                    }
                }
            }
        }
    }
}
