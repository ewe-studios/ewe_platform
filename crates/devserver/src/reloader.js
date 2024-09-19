console.log("Welcome to the sse reloader script");

const reload_signals = new EventSource("/static/sse/reload");

reload_signals.addEventListener("reload", (event) => {
  console.log("Received reload event signal", event);
  window.location.reload();
  console.log("Reload sent!");
});

reload_signals.addEventListener("message", (event) => {
  console.log("Received generic event check your setup", event);
});
