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


// ---------------------------------------------------------------------------
// Parameter binding
//
// The D1 Workers binding documents this JavaScript-to-SQLite mapping
// ("Type conversion" in the D1 Workers Binding API reference,
// https://developers.cloudflare.com/d1/worker-api/): null -> NULL,
// Number -> REAL (an integer-valued Number is INTEGER only through column
// affinity), String -> TEXT, Boolean -> INTEGER, ArrayBuffer -> BLOB;
// BigInt and undefined are not supported. Checked with wrangler 4.156.2
// (workerd): `bind(7)` makes `typeof(?1)` = 'real', and `bind(7n)` throws
// D1_TYPE_ERROR.
//
// The clients of this stub send JSON, where `7` and `7.0` differ. Hosted D1
// stores a JSON integer as INTEGER, so the stub keeps that meaning: a JSON
// integer literal is bound as `CAST(?N AS INTEGER)` (an integer outside the
// double-safe range is bound as its decimal text, which CAST reads exactly),
// a fractional number stays a REAL, null stays NULL and a string stays TEXT.
// The stub reads the literal's source text (`JSON.parse` reviver context) to
// tell `7` from `7.0`.
// ---------------------------------------------------------------------------

class SqlInteger {
  constructor(text) {
    this.text = text;
  }
}

function parseRequestJson(text) {
  return JSON.parse(text, (_key, value, context) =>
    typeof value === "number" && /^-?\d+$/.test(context?.source ?? "")
      ? new SqlInteger(context.source)
      : value,
  );
}

// Rewrite every `?` / `?N` placeholder whose parameter is a JSON integer into
// `CAST(?N AS INTEGER)`. Quoted text and comments are copied unchanged.
function rewriteIntegerPlaceholders(sql, params) {
  let out = "";
  let largest = 0;
  let i = 0;
  while (i < sql.length) {
    const c = sql[i];
    if (c === "'" || c === '"' || c === "`" || c === "[") {
      const close = c === "[" ? "]" : c;
      let j = i + 1;
      while (j < sql.length) {
        if (sql[j] === close) {
          if (close !== "]" && sql[j + 1] === close) {
            j += 2; // doubled quote
            continue;
          }
          break;
        }
        j++;
      }
      out += sql.slice(i, j + 1);
      i = j + 1;
    } else if (c === "-" && sql[i + 1] === "-") {
      const end = sql.indexOf("\n", i);
      const stop = end === -1 ? sql.length : end;
      out += sql.slice(i, stop);
      i = stop;
    } else if (c === "/" && sql[i + 1] === "*") {
      const end = sql.indexOf("*/", i + 2);
      const stop = end === -1 ? sql.length : end + 2;
      out += sql.slice(i, stop);
      i = stop;
    } else if (c === "?") {
      let j = i + 1;
      while (j < sql.length && sql[j] >= "0" && sql[j] <= "9") j++;
      const digits = sql.slice(i + 1, j);
      // SQLite numbers a bare `?` one above the largest number so far.
      const index = digits === "" ? largest + 1 : Number(digits);
      largest = Math.max(largest, index);
      out += params[index - 1] instanceof SqlInteger
        ? `CAST(?${index} AS INTEGER)`
        : sql.slice(i, j);
      i = j;
    } else {
      out += c;
      i++;
    }
  }
  return out;
}

function bindableValue(value) {
  if (value instanceof SqlInteger) {
    const n = Number(value.text);
    return Number.isSafeInteger(n) ? n : value.text;
  }
  return value;
}

function prepareStatement(env, sql, params) {
  if (params.length === 0) {
    return env.DB.prepare(sql);
  }
  return env.DB.prepare(rewriteIntegerPlaceholders(sql, params)).bind(
    ...params.map(bindableValue),
  );
}

async function handleD1Query(request, env) {
  let body;
  try {
    body = parseRequestJson(await request.text());
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
    const stmt = prepareStatement(env, sql, params);
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
      return prepareStatement(env, entry.sql, params);
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
