// Local wrangler worker used by `mise run test:cf:*`.
//
// This worker emulates just enough of the Cloudflare REST API to let the
// foundation_db D1 / R2 integration tests run against a local wrangler dev
// instance. The Rust clients call:
//
//   POST   /accounts/:a/d1/database/:d/query          (D1: `{sql, params}` or `{batch: [...]}`)
//   PUT    /accounts/:a/r2/buckets/:b/objects/:key    (R2)
//   GET    /accounts/:a/r2/buckets/:b/objects/:key
//   DELETE /accounts/:a/r2/buckets/:b/objects/:key
//   HEAD   /accounts/:a/r2/buckets/:b/objects/:key
//   GET    /accounts/:a/r2/buckets/:b/objects         (R2 object listing)
//
// Requests are routed to the `DB` / `BUCKET` bindings declared in
// wrangler.toml. Responses mimic the Cloudflare API shape the clients expect.

const D1_QUERY_RE = /^\/accounts\/[^/]+\/d1\/database\/[^/]+\/query$/;
const R2_OBJECT_RE = /^\/accounts\/[^/]+\/r2\/buckets\/[^/]+\/objects\/(.+)$/;
const R2_LIST_RE = /^\/accounts\/[^/]+\/r2\/buckets\/[^/]+\/objects$/;

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

    if (R2_LIST_RE.test(path) && request.method === "GET") {
      return handleR2List(request, env);
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

  // The D1 REST `batch` shape: `{ "batch": [ { sql, params? }, ... ] }`. The
  // D1 binding's `batch()` runs the statements in one transaction, so a failed
  // statement rolls all of them back, as Cloudflare documents for the REST API.
  if (Array.isArray(body.batch)) {
    return handleD1Batch(body.batch, env);
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

async function handleD1Batch(batch, env) {
  for (const entry of batch) {
    if (!entry || typeof entry.sql !== "string") {
      return cfError(400, "missing 'sql' field in a batch entry");
    }
  }

  try {
    const statements = batch.map((entry) => {
      const params = Array.isArray(entry.params) ? entry.params : [];
      const stmt = env.DB.prepare(entry.sql);
      return params.length > 0 ? stmt.bind(...params) : stmt;
    });
    const results = await env.DB.batch(statements);

    return new Response(
      JSON.stringify({
        result: results.map((result) => ({
          results: result.results || [],
          meta: result.meta || {},
          success: true,
        })),
        success: true,
        errors: [],
        messages: [],
      }),
      {
        status: 200,
        headers: { "content-type": "application/json" },
      },
    );
  } catch (e) {
    return cfError(400, `D1 batch failed: ${e?.message || e}`);
  }
}

async function handleR2List(request, env) {
  const url = new URL(request.url);
  const prefix = url.searchParams.get("prefix") || "";
  const cursor = url.searchParams.get("cursor") || undefined;

  const listed = await env.BUCKET.list({ prefix, cursor });
  const objects = (listed.objects || []).map((object) => ({ key: object.key }));

  return new Response(
    JSON.stringify({
      success: true,
      errors: [],
      messages: [],
      result: objects,
      result_info: {
        is_truncated: listed.truncated || false,
        cursor: listed.cursor || "",
      },
    }),
    {
      status: 200,
      headers: { "content-type": "application/json" },
    },
  );
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
