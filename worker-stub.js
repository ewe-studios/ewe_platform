// Local wrangler worker used by `mise run test:cf:*`.
//
// This worker emulates just enough of the Cloudflare REST API to let the
// foundation_db D1 / R2 integration tests run against a local wrangler dev
// instance. The Rust clients call:
//
//   POST   /accounts/:a/d1/database/:d/query          (D1)
//   PUT    /accounts/:a/r2/buckets/:b/objects/:key    (R2)
//   GET    /accounts/:a/r2/buckets/:b/objects/:key
//   DELETE /accounts/:a/r2/buckets/:b/objects/:key
//   HEAD   /accounts/:a/r2/buckets/:b/objects/:key
//
// Requests are routed to the `DB` / `BUCKET` bindings declared in
// wrangler.toml. Responses mimic the Cloudflare API shape the clients expect.

const D1_QUERY_RE = /^\/accounts\/[^/]+\/d1\/database\/[^/]+\/query$/;
const R2_OBJECT_RE = /^\/accounts\/[^/]+\/r2\/buckets\/[^/]+\/objects\/(.+)$/;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const path = url.pathname;

    if (path === "/" || path === "/health") {
      return new Response("Foundation DB Test Worker", {
        status: 200,
        headers: { "content-type": "text/plain" },
      });
    }

    if (D1_QUERY_RE.test(path) && request.method === "POST") {
      return handleD1Query(request, env);
    }

    const r2Match = path.match(R2_OBJECT_RE);
    if (r2Match) {
      const key = decodeURIComponent(r2Match[1]);
      return handleR2(request, env, key);
    }

    return new Response(
      JSON.stringify({ success: false, error: "not found", path }),
      {
        status: 404,
        headers: { "content-type": "application/json" },
      },
    );
  },
};

async function handleD1Query(request, env) {
  let body;
  try {
    body = await request.json();
  } catch (e) {
    return cfError(400, `invalid JSON body: ${e}`);
  }

  const sql = body.sql;
  const params = Array.isArray(body.params) ? body.params : [];

  if (typeof sql !== "string") {
    return cfError(400, "missing 'sql' field");
  }

  try {
    const stmt = params.length > 0
      ? env.DB.prepare(sql).bind(...params)
      : env.DB.prepare(sql);
    const result = await stmt.all();

    const responseBody = JSON.stringify({
      result: [
        {
          results: result.results || [],
          meta: result.meta || {},
          success: true,
        },
      ],
      success: true,
      errors: [],
      messages: [],
    });

    return new Response(responseBody, {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  } catch (e) {
    return cfError(400, `D1 execution failed: ${e?.message || e}`);
  }
}

async function handleR2(request, env, key) {
  const method = request.method;

  if (method === "PUT") {
    const body = await request.arrayBuffer();
    await env.BUCKET.put(key, body);
    return new Response(null, {
      status: 200,
      headers: {
        "content-length": "0",
        "connection": "close"
      }
    });
  }

  if (method === "GET") {
    const obj = await env.BUCKET.get(key);
    if (!obj) return new Response(null, {
      status: 404,
      headers: { "content-length": "0", "connection": "close" }
    });
    const buf = await obj.arrayBuffer();
    return new Response(buf, {
      status: 200,
      headers: { "content-type": "application/octet-stream" },
    });
  }

  if (method === "HEAD") {
    const obj = await env.BUCKET.head(key);
    if (!obj) return new Response(null, {
      status: 404,
      headers: { "content-length": "0", "connection": "close" }
    });
    return new Response(null, {
      status: 200,
      headers: { "content-length": "0", "connection": "close" }
    });
  }

  if (method === "DELETE") {
    await env.BUCKET.delete(key);
    return new Response(null, {
      status: 204,
      headers: {
        "content-length": "0",
        "connection": "close"
      }
    });
  }

  return new Response("method not allowed", { status: 405 });
}

function cfError(status, message) {
  return new Response(
    JSON.stringify({
      result: null,
      success: false,
      errors: [{ message }],
      messages: [],
    }),
    {
      status,
      headers: { "content-type": "application/json" },
    },
  );
}
