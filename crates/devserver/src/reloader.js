console.log("Welcome to the sse reloader script");

const reload_signals = new EventSource("/static/sse/reload");

reload_signals.addEventListener("reload", (event) => {
  console.log("Received reload event signal", event);

  const endpoint = window.location.href;
  fetch(endpoint)
    .then((response) => {
      if (response.ok) {
        window.location.reload();
      } else {
        console.log(
          `Failed to reach endpoint: ${endpoint} due to response`,
          response,
        );
      }
    })
    .catch((e) => {
      console.log(`Failed to reach page: ${endpoint}`);
    });
});

reload_signals.addEventListener("message", (event) => {
  console.log("Received generic event check your setup", event);
});
