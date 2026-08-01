const evtSource = new EventSource("/.internal/live-reload");

evtSource.addEventListener("reload", (event) => {
  // only reload the CSS stylesheets if only CSS files were changed
  if (event.data === "css") {
    for (let stylesheet of document.querySelectorAll("link[rel=stylesheet]")) {
      const url = new URL(stylesheet.href);
      // modify the date query parameter so that the stylesheet gets reloaded
      // https://stackoverflow.com/questions/2024486/is-there-an-easy-way-to-reload-css-without-reloading-the-page
      url.searchParams.set("date", Date.now());
      stylesheet.href = url.toString();
    }
  } else {
    // reload the whole site
    window.location.reload();
  }
});
