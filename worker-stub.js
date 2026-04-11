// Stub worker for local wrangler dev testing
// This file is only used when running wrangler dev for R2/D1 integration tests

export default {
  async fetch(request, env, ctx) {
    return new Response('Foundation DB Test Worker', {
      status: 200,
      headers: { 'Content-Type': 'text/plain' }
    });
  }
};
