// An empty worker. `wrangler dev --local` needs an entry point; the tests talk
// to wrangler's local explorer API (/cdn-cgi/local/explorer/api), not to this.
export default {
  async fetch() {
    return new Response("ok");
  },
};
