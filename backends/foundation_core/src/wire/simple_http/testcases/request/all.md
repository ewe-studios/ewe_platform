Imagine we want to extract the http blocks and write a test for each header transforming the header name into a valid rust test function where the function has a variable called message that contains the body of the http code block and ensure to preserve newlines and carriage return in the text, collapsing it into a single string in double quotes. Write out the test function for me in rust. Each main header is a test module on its own, so example: The `Transfer-Encoding-Header` is a test module on it's own.

Wrap it in a root module called "http_requests_compliance".


URI
===

## Quotes in URI

<!-- meta={"type": "request"} -->
```http
GET /with_"lovely"_quotes?foo=\"bar\" HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=33 span[url]="/with_"lovely"_quotes?foo=\"bar\""
off=38 url complete
off=38 len=4 span[protocol]="HTTP"
off=42 protocol complete
off=43 len=3 span[version]="1.1"
off=46 version complete
off=50 headers complete method=1 v=1/1 flags=0 content_length=0
off=50 message complete
```

## Query URL with question mark

Some clients include `?` characters in query strings.

<!-- meta={"type": "request"} -->
```http
GET /test.cgi?foo=bar?baz HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=21 span[url]="/test.cgi?foo=bar?baz"
off=26 url complete
off=26 len=4 span[protocol]="HTTP"
off=30 protocol complete
off=31 len=3 span[version]="1.1"
off=34 version complete
off=38 headers complete method=1 v=1/1 flags=0 content_length=0
off=38 message complete
```

## Host terminated by a query string

<!-- meta={"type": "request"} -->
```http
GET http://hypnotoad.org?hail=all HTTP/1.1\r\n


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=29 span[url]="http://hypnotoad.org?hail=all"
off=34 url complete
off=34 len=4 span[protocol]="HTTP"
off=38 protocol complete
off=39 len=3 span[version]="1.1"
off=42 version complete
off=46 headers complete method=1 v=1/1 flags=0 content_length=0
off=46 message complete
```

## `host:port` terminated by a query string

<!-- meta={"type": "request"} -->
```http
GET http://hypnotoad.org:1234?hail=all HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=34 span[url]="http://hypnotoad.org:1234?hail=all"
off=39 url complete
off=39 len=4 span[protocol]="HTTP"
off=43 protocol complete
off=44 len=3 span[version]="1.1"
off=47 version complete
off=51 headers complete method=1 v=1/1 flags=0 content_length=0
off=51 message complete
```

## Query URL with vertical bar character

It should be allowed to have vertical bar symbol in URI: `|`.

See: https://github.com/nodejs/node/issues/27584

<!-- meta={"type": "request"} -->
```http
GET /test.cgi?query=| HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=17 span[url]="/test.cgi?query=|"
off=22 url complete
off=22 len=4 span[protocol]="HTTP"
off=26 protocol complete
off=27 len=3 span[version]="1.1"
off=30 version complete
off=34 headers complete method=1 v=1/1 flags=0 content_length=0
off=34 message complete
```

## `host:port` terminated by a space

<!-- meta={"type": "request"} -->
```http
GET http://hypnotoad.org:1234 HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=25 span[url]="http://hypnotoad.org:1234"
off=30 url complete
off=30 len=4 span[protocol]="HTTP"
off=34 protocol complete
off=35 len=3 span[version]="1.1"
off=38 version complete
off=42 headers complete method=1 v=1/1 flags=0 content_length=0
off=42 message complete
```

## Disallow UTF-8 in URI path in strict mode

<!-- meta={"type": "request",  "noScan": true} -->
```http
GET /δ¶/δt/pope?q=1#narf HTTP/1.1
Host: github.com


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=5 error code=7 reason="Invalid char in url path"
```

## Fragment in URI

<!-- meta={"type": "request"} -->
```http
GET /forums/1/topics/2375?page=1#posts-17408 HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=40 span[url]="/forums/1/topics/2375?page=1#posts-17408"
off=45 url complete
off=45 len=4 span[protocol]="HTTP"
off=49 protocol complete
off=50 len=3 span[version]="1.1"
off=53 version complete
off=57 headers complete method=1 v=1/1 flags=0 content_length=0
off=57 message complete
```

## Underscore in hostname

<!-- meta={"type": "request"} -->
```http
CONNECT home_0.netscape.com:443 HTTP/1.0
User-agent: Mozilla/1.1N
Proxy-authorization: basic aGVsbG86d29ybGQ=


```

```log
off=0 message begin
off=0 len=7 span[method]="CONNECT"
off=7 method complete
off=8 len=23 span[url]="home_0.netscape.com:443"
off=32 url complete
off=32 len=4 span[protocol]="HTTP"
off=36 protocol complete
off=37 len=3 span[version]="1.0"
off=40 version complete
off=42 len=10 span[header_field]="User-agent"
off=53 header_field complete
off=54 len=12 span[header_value]="Mozilla/1.1N"
off=68 header_value complete
off=68 len=19 span[header_field]="Proxy-authorization"
off=88 header_field complete
off=89 len=22 span[header_value]="basic aGVsbG86d29ybGQ="
off=113 header_value complete
off=115 headers complete method=5 v=1/0 flags=0 content_length=0
off=115 message complete
off=115 error code=22 reason="Pause on CONNECT/Upgrade"
```

## `host:port` and basic auth

<!-- meta={"type": "request"} -->
```http
GET http://a%12:b!&*$@hypnotoad.org:1234/toto HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=41 span[url]="http://a%12:b!&*$@hypnotoad.org:1234/toto"
off=46 url complete
off=46 len=4 span[protocol]="HTTP"
off=50 protocol complete
off=51 len=3 span[version]="1.1"
off=54 version complete
off=58 headers complete method=1 v=1/1 flags=0 content_length=0
off=58 message complete
```

## Space in URI

<!-- meta={"type": "request", "noScan": true} -->
```http
GET /foo bar/ HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=4 span[url]="/foo"
off=9 url complete
off=9 len=0 span[protocol]=""
off=9 error code=8 reason="Expected HTTP/, RTSP/ or ICE/"
```



Transfer-Encoding header
========================

## `chunked`

### Parsing and setting flag

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=208 content_length=0
```

### Parse chunks with lowercase size

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked

a
0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=208 content_length=0
off=52 chunk header len=10
off=52 len=10 span[body]="0123456789"
off=64 chunk complete
off=67 chunk header len=0
off=69 chunk complete
off=69 message complete
```

### Parse chunks with uppercase size

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked

A
0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=208 content_length=0
off=52 chunk header len=10
off=52 len=10 span[body]="0123456789"
off=64 chunk complete
off=67 chunk header len=0
off=69 chunk complete
off=69 message complete
```

### POST with `Transfer-Encoding: chunked`

<!-- meta={"type": "request"} -->
```http
POST /post_chunked_all_your_base HTTP/1.1
Transfer-Encoding: chunked

1e
all your base are belong to us
0


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=27 span[url]="/post_chunked_all_your_base"
off=33 url complete
off=33 len=4 span[protocol]="HTTP"
off=37 protocol complete
off=38 len=3 span[version]="1.1"
off=41 version complete
off=43 len=17 span[header_field]="Transfer-Encoding"
off=61 header_field complete
off=62 len=7 span[header_value]="chunked"
off=71 header_value complete
off=73 headers complete method=3 v=1/1 flags=208 content_length=0
off=77 chunk header len=30
off=77 len=30 span[body]="all your base are belong to us"
off=109 chunk complete
off=112 chunk header len=0
off=114 chunk complete
off=114 message complete
```

### Two chunks and triple zero prefixed end chunk

<!-- meta={"type": "request"} -->
```http
POST /two_chunks_mult_zero_end HTTP/1.1
Transfer-Encoding: chunked

5
hello
6
 world
000


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=25 span[url]="/two_chunks_mult_zero_end"
off=31 url complete
off=31 len=4 span[protocol]="HTTP"
off=35 protocol complete
off=36 len=3 span[version]="1.1"
off=39 version complete
off=41 len=17 span[header_field]="Transfer-Encoding"
off=59 header_field complete
off=60 len=7 span[header_value]="chunked"
off=69 header_value complete
off=71 headers complete method=3 v=1/1 flags=208 content_length=0
off=74 chunk header len=5
off=74 len=5 span[body]="hello"
off=81 chunk complete
off=84 chunk header len=6
off=84 len=6 span[body]=" world"
off=92 chunk complete
off=97 chunk header len=0
off=99 chunk complete
off=99 message complete
```

### Trailing headers

<!-- meta={"type": "request"} -->
```http
POST /chunked_w_trailing_headers HTTP/1.1
Transfer-Encoding: chunked

5
hello
6
 world
0
Vary: *
Content-Type: text/plain


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=27 span[url]="/chunked_w_trailing_headers"
off=33 url complete
off=33 len=4 span[protocol]="HTTP"
off=37 protocol complete
off=38 len=3 span[version]="1.1"
off=41 version complete
off=43 len=17 span[header_field]="Transfer-Encoding"
off=61 header_field complete
off=62 len=7 span[header_value]="chunked"
off=71 header_value complete
off=73 headers complete method=3 v=1/1 flags=208 content_length=0
off=76 chunk header len=5
off=76 len=5 span[body]="hello"
off=83 chunk complete
off=86 chunk header len=6
off=86 len=6 span[body]=" world"
off=94 chunk complete
off=97 chunk header len=0
off=97 len=4 span[header_field]="Vary"
off=102 header_field complete
off=103 len=1 span[header_value]="*"
off=106 header_value complete
off=106 len=12 span[header_field]="Content-Type"
off=119 header_field complete
off=120 len=10 span[header_value]="text/plain"
off=132 header_value complete
off=134 chunk complete
off=134 message complete
```

### Chunk extensions

<!-- meta={"type": "request"} -->
```http
POST /chunked_w_unicorns_after_length HTTP/1.1
Transfer-Encoding: chunked

5;ilovew3;somuchlove=aretheseparametersfor;another=withvalue
hello
6;blahblah;blah
 world
0

```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=32 span[url]="/chunked_w_unicorns_after_length"
off=38 url complete
off=38 len=4 span[protocol]="HTTP"
off=42 protocol complete
off=43 len=3 span[version]="1.1"
off=46 version complete
off=48 len=17 span[header_field]="Transfer-Encoding"
off=66 header_field complete
off=67 len=7 span[header_value]="chunked"
off=76 header_value complete
off=78 headers complete method=3 v=1/1 flags=208 content_length=0
off=80 len=7 span[chunk_extension_name]="ilovew3"
off=88 chunk_extension_name complete
off=88 len=10 span[chunk_extension_name]="somuchlove"
off=99 chunk_extension_name complete
off=99 len=21 span[chunk_extension_value]="aretheseparametersfor"
off=121 chunk_extension_value complete
off=121 len=7 span[chunk_extension_name]="another"
off=129 chunk_extension_name complete
off=129 len=9 span[chunk_extension_value]="withvalue"
off=139 chunk_extension_value complete
off=140 chunk header len=5
off=140 len=5 span[body]="hello"
off=147 chunk complete
off=149 len=8 span[chunk_extension_name]="blahblah"
off=158 chunk_extension_name complete
off=158 len=4 span[chunk_extension_name]="blah"
off=163 chunk_extension_name complete
off=164 chunk header len=6
off=164 len=6 span[body]=" world"
off=172 chunk complete
off=175 chunk header len=0
```

### No semicolon before chunk extensions

<!-- meta={"type": "request"} -->
```http
POST /chunked_w_unicorns_after_length HTTP/1.1
Host: localhost
Transfer-encoding: chunked

2 erfrferferf
aa
0 rrrr


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=32 span[url]="/chunked_w_unicorns_after_length"
off=38 url complete
off=38 len=4 span[protocol]="HTTP"
off=42 protocol complete
off=43 len=3 span[version]="1.1"
off=46 version complete
off=48 len=4 span[header_field]="Host"
off=53 header_field complete
off=54 len=9 span[header_value]="localhost"
off=65 header_value complete
off=65 len=17 span[header_field]="Transfer-encoding"
off=83 header_field complete
off=84 len=7 span[header_value]="chunked"
off=93 header_value complete
off=95 headers complete method=3 v=1/1 flags=208 content_length=0
off=97 error code=12 reason="Invalid character in chunk size"
```

### No extension after semicolon

<!-- meta={"type": "request"} -->
```http
POST /chunked_w_unicorns_after_length HTTP/1.1
Host: localhost
Transfer-encoding: chunked

2;
aa
0


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=32 span[url]="/chunked_w_unicorns_after_length"
off=38 url complete
off=38 len=4 span[protocol]="HTTP"
off=42 protocol complete
off=43 len=3 span[version]="1.1"
off=46 version complete
off=48 len=4 span[header_field]="Host"
off=53 header_field complete
off=54 len=9 span[header_value]="localhost"
off=65 header_value complete
off=65 len=17 span[header_field]="Transfer-encoding"
off=83 header_field complete
off=84 len=7 span[header_value]="chunked"
off=93 header_value complete
off=95 headers complete method=3 v=1/1 flags=208 content_length=0
off=98 error code=2 reason="Invalid character in chunk extensions"
```


### Chunk extensions quoting

<!-- meta={"type": "request"} -->
```http
POST /chunked_w_unicorns_after_length HTTP/1.1
Transfer-Encoding: chunked

5;ilovew3="I \"love\"; \\extensions\\";somuchlove="aretheseparametersfor";blah;foo=bar
hello
6;blahblah;blah
 world
0

```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=32 span[url]="/chunked_w_unicorns_after_length"
off=38 url complete
off=38 len=4 span[protocol]="HTTP"
off=42 protocol complete
off=43 len=3 span[version]="1.1"
off=46 version complete
off=48 len=17 span[header_field]="Transfer-Encoding"
off=66 header_field complete
off=67 len=7 span[header_value]="chunked"
off=76 header_value complete
off=78 headers complete method=3 v=1/1 flags=208 content_length=0
off=80 len=7 span[chunk_extension_name]="ilovew3"
off=88 chunk_extension_name complete
off=88 len=28 span[chunk_extension_value]=""I \"love\"; \\extensions\\""
off=116 chunk_extension_value complete
off=117 len=10 span[chunk_extension_name]="somuchlove"
off=128 chunk_extension_name complete
off=128 len=23 span[chunk_extension_value]=""aretheseparametersfor""
off=151 chunk_extension_value complete
off=152 len=4 span[chunk_extension_name]="blah"
off=157 chunk_extension_name complete
off=157 len=3 span[chunk_extension_name]="foo"
off=161 chunk_extension_name complete
off=161 len=3 span[chunk_extension_value]="bar"
off=165 chunk_extension_value complete
off=166 chunk header len=5
off=166 len=5 span[body]="hello"
off=173 chunk complete
off=175 len=8 span[chunk_extension_name]="blahblah"
off=184 chunk_extension_name complete
off=184 len=4 span[chunk_extension_name]="blah"
off=189 chunk_extension_name complete
off=190 chunk header len=6
off=190 len=6 span[body]=" world"
off=198 chunk complete
off=201 chunk header len=0
```


### Unbalanced chunk extensions quoting

<!-- meta={"type": "request"} -->
```http
POST /chunked_w_unicorns_after_length HTTP/1.1
Transfer-Encoding: chunked

5;ilovew3="abc";somuchlove="def; ghi
hello
6;blahblah;blah
 world
0

```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=32 span[url]="/chunked_w_unicorns_after_length"
off=38 url complete
off=38 len=4 span[protocol]="HTTP"
off=42 protocol complete
off=43 len=3 span[version]="1.1"
off=46 version complete
off=48 len=17 span[header_field]="Transfer-Encoding"
off=66 header_field complete
off=67 len=7 span[header_value]="chunked"
off=76 header_value complete
off=78 headers complete method=3 v=1/1 flags=208 content_length=0
off=80 len=7 span[chunk_extension_name]="ilovew3"
off=88 chunk_extension_name complete
off=88 len=5 span[chunk_extension_value]=""abc""
off=93 chunk_extension_value complete
off=94 len=10 span[chunk_extension_name]="somuchlove"
off=105 chunk_extension_name complete
off=105 len=9 span[chunk_extension_value]=""def; ghi"
off=115 error code=2 reason="Invalid character in chunk extensions quoted value"
```

## Ignoring `pigeons`

Requests cannot have invalid `Transfer-Encoding`. It is impossible to determine
their body size. Not erroring would make HTTP smuggling attacks possible.

<!-- meta={"type": "request", "noScan": true} -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: pigeons


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="pigeons"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=200 content_length=0
off=49 error code=15 reason="Request has invalid `Transfer-Encoding`"
```

## POST with `Transfer-Encoding` and `Content-Length`

<!-- meta={"type": "request"} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: identity
Content-Length: 5

World
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=8 span[header_value]="identity"
off=96 header_value complete
off=96 len=14 span[header_field]="Content-Length"
off=111 header_field complete
off=111 error code=11 reason="Content-Length can't be present with Transfer-Encoding"
```

## POST with `Transfer-Encoding` and `Content-Length` (lenient)

TODO(indutny): should we allow it even in lenient mode? (Consider disabling
this).

NOTE: `Content-Length` is ignored when `Transfer-Encoding` is present. Messages
(in lenient mode) are read until EOF.

<!-- meta={"type": "request-lenient-chunked-length"} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: identity
Content-Length: 1

World
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=8 span[header_value]="identity"
off=96 header_value complete
off=96 len=14 span[header_field]="Content-Length"
off=111 header_field complete
off=112 len=1 span[header_value]="1"
off=115 header_value complete
off=117 headers complete method=3 v=1/1 flags=220 content_length=1
off=117 len=5 span[body]="World"
```

## POST with empty `Transfer-Encoding` and `Content-Length` (lenient)

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Host: foo
Content-Length: 10
Transfer-Encoding:
Transfer-Encoding:
Transfer-Encoding:

2
AA
0
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=3 span[header_value]="foo"
off=28 header_value complete
off=28 len=14 span[header_field]="Content-Length"
off=43 header_field complete
off=44 len=2 span[header_value]="10"
off=48 header_value complete
off=48 len=17 span[header_field]="Transfer-Encoding"
off=66 header_field complete
off=66 error code=15 reason="Transfer-Encoding can't be present with Content-Length"
```

## POST with `chunked` before other transfer coding names

<!-- meta={"type": "request", "noScan": true} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: chunked, deflate

World
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=7 span[header_value]="chunked"
off=94 error code=15 reason="Invalid `Transfer-Encoding` header value"
```

## POST with `chunked` and duplicate transfer-encoding

<!-- meta={"type": "request", "noScan": true} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: chunked
Transfer-Encoding: deflate

World
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=7 span[header_value]="chunked"
off=95 header_value complete
off=95 len=17 span[header_field]="Transfer-Encoding"
off=113 header_field complete
off=114 len=0 span[header_value]=""
off=115 error code=15 reason="Invalid `Transfer-Encoding` header value"
```

## POST with `chunked` before other transfer-coding (lenient)

<!-- meta={"type": "request-lenient-transfer-encoding"} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: chunked, deflate

World
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=16 span[header_value]="chunked, deflate"
off=104 header_value complete
off=106 headers complete method=3 v=1/1 flags=200 content_length=0
off=106 len=5 span[body]="World"
```

## POST with `chunked` and duplicate transfer-encoding (lenient)

<!-- meta={"type": "request-lenient-transfer-encoding"} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: chunked
Transfer-Encoding: deflate

World
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=7 span[header_value]="chunked"
off=95 header_value complete
off=95 len=17 span[header_field]="Transfer-Encoding"
off=113 header_field complete
off=114 len=7 span[header_value]="deflate"
off=123 header_value complete
off=125 headers complete method=3 v=1/1 flags=200 content_length=0
off=125 len=5 span[body]="World"
```

## POST with `chunked` as last transfer-encoding

<!-- meta={"type": "request"} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: deflate, chunked

5
World
0


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=16 span[header_value]="deflate, chunked"
off=104 header_value complete
off=106 headers complete method=3 v=1/1 flags=208 content_length=0
off=109 chunk header len=5
off=109 len=5 span[body]="World"
off=116 chunk complete
off=119 chunk header len=0
off=121 chunk complete
off=121 message complete
```

## POST with `chunked` as last transfer-encoding (multiple headers)

<!-- meta={"type": "request"} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: deflate
Transfer-Encoding: chunked

5
World
0


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=7 span[header_value]="deflate"
off=95 header_value complete
off=95 len=17 span[header_field]="Transfer-Encoding"
off=113 header_field complete
off=114 len=7 span[header_value]="chunked"
off=123 header_value complete
off=125 headers complete method=3 v=1/1 flags=208 content_length=0
off=128 chunk header len=5
off=128 len=5 span[body]="World"
off=135 chunk complete
off=138 chunk header len=0
off=140 chunk complete
off=140 message complete
```

## POST with `chunkedchunked` as transfer-encoding

<!-- meta={"type": "request"} -->
```http
POST /post_identity_body_world?q=search#hey HTTP/1.1
Accept: */*
Transfer-Encoding: chunkedchunked

5
World
0


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=38 span[url]="/post_identity_body_world?q=search#hey"
off=44 url complete
off=44 len=4 span[protocol]="HTTP"
off=48 protocol complete
off=49 len=3 span[version]="1.1"
off=52 version complete
off=54 len=6 span[header_field]="Accept"
off=61 header_field complete
off=62 len=3 span[header_value]="*/*"
off=67 header_value complete
off=67 len=17 span[header_field]="Transfer-Encoding"
off=85 header_field complete
off=86 len=14 span[header_value]="chunkedchunked"
off=102 header_value complete
off=104 headers complete method=3 v=1/1 flags=200 content_length=0
off=104 error code=15 reason="Request has invalid `Transfer-Encoding`"
```

## Missing last-chunk

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked

3
foo


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=208 content_length=0
off=52 chunk header len=3
off=52 len=3 span[body]="foo"
off=57 chunk complete
off=57 error code=12 reason="Invalid character in chunk size"
```

## Validate chunk parameters

<!-- meta={"type": "request" } -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked

3 \n  \r\n\
foo


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=208 content_length=0
off=51 error code=12 reason="Invalid character in chunk size"
```

## Invalid OBS fold after chunked value

<!-- meta={"type": "request-lenient-headers" } -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked
  abc

5
World
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 len=5 span[header_value]="  abc"
off=54 header_value complete
off=56 headers complete method=4 v=1/1 flags=200 content_length=0
off=56 error code=15 reason="Request has invalid `Transfer-Encoding`"
```

### Chunk header not terminated by CRLF

<!-- meta={"type": "request" } -->

```http
GET / HTTP/1.1
Host: a
Connection: close 
Transfer-Encoding: chunked 

5\r\r;ABCD
34
E
0

GET / HTTP/1.1 
Host: a
Content-Length: 5

0

```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Host"
off=21 header_field complete
off=22 len=1 span[header_value]="a"
off=25 header_value complete
off=25 len=10 span[header_field]="Connection"
off=36 header_field complete
off=37 len=6 span[header_value]="close "
off=45 header_value complete
off=45 len=17 span[header_field]="Transfer-Encoding"
off=63 header_field complete
off=64 len=8 span[header_value]="chunked "
off=74 header_value complete
off=76 headers complete method=1 v=1/1 flags=20a content_length=0
off=78 error code=2 reason="Expected LF after chunk size"
```

### Chunk header not terminated by CRLF (lenient)

<!-- meta={"type": "request-lenient-optional-lf-after-cr" } -->

```http
GET / HTTP/1.1
Host: a
Connection: close 
Transfer-Encoding: chunked 

6\r\r;ABCD
33
E
0

GET / HTTP/1.1 
Host: a
Content-Length: 5
0


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Host"
off=21 header_field complete
off=22 len=1 span[header_value]="a"
off=25 header_value complete
off=25 len=10 span[header_field]="Connection"
off=36 header_field complete
off=37 len=6 span[header_value]="close "
off=45 header_value complete
off=45 len=17 span[header_field]="Transfer-Encoding"
off=63 header_field complete
off=64 len=8 span[header_value]="chunked "
off=74 header_value complete
off=76 headers complete method=1 v=1/1 flags=20a content_length=0
off=78 chunk header len=6
off=78 len=1 span[body]=cr
off=79 len=5 span[body]=";ABCD"
off=86 chunk complete
off=90 chunk header len=51
off=90 len=1 span[body]="E"
off=91 len=1 span[body]=cr
off=92 len=1 span[body]=lf
off=93 len=1 span[body]="0"
off=94 len=1 span[body]=cr
off=95 len=1 span[body]=lf
off=96 len=1 span[body]=cr
off=97 len=1 span[body]=lf
off=98 len=15 span[body]="GET / HTTP/1.1 "
off=113 len=1 span[body]=cr
off=114 len=1 span[body]=lf
off=115 len=7 span[body]="Host: a"
off=122 len=1 span[body]=cr
off=123 len=1 span[body]=lf
off=124 len=17 span[body]="Content-Length: 5"
off=143 chunk complete
off=146 chunk header len=0
off=148 chunk complete
off=148 message complete
```

### Chunk data not terminated by CRLF

<!-- meta={"type": "request" } -->

```http
GET / HTTP/1.1
Host: a
Connection: close 
Transfer-Encoding: chunked 

5
ABCDE0

```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Host"
off=21 header_field complete
off=22 len=1 span[header_value]="a"
off=25 header_value complete
off=25 len=10 span[header_field]="Connection"
off=36 header_field complete
off=37 len=6 span[header_value]="close "
off=45 header_value complete
off=45 len=17 span[header_field]="Transfer-Encoding"
off=63 header_field complete
off=64 len=8 span[header_value]="chunked "
off=74 header_value complete
off=76 headers complete method=1 v=1/1 flags=20a content_length=0
off=79 chunk header len=5
off=79 len=5 span[body]="ABCDE"
off=84 error code=2 reason="Expected LF after chunk data"
```

### Chunk data not terminated by CRLF (lenient)

<!-- meta={"type": "request-lenient-optional-crlf-after-chunk" } -->

```http
GET / HTTP/1.1
Host: a
Connection: close 
Transfer-Encoding: chunked 

5
ABCDE0

```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Host"
off=21 header_field complete
off=22 len=1 span[header_value]="a"
off=25 header_value complete
off=25 len=10 span[header_field]="Connection"
off=36 header_field complete
off=37 len=6 span[header_value]="close "
off=45 header_value complete
off=45 len=17 span[header_field]="Transfer-Encoding"
off=63 header_field complete
off=64 len=8 span[header_value]="chunked "
off=74 header_value complete
off=76 headers complete method=1 v=1/1 flags=20a content_length=0
off=79 chunk header len=5
off=79 len=5 span[body]="ABCDE"
off=84 chunk complete
off=87 chunk header len=0
```

## Space after chunk header

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked

a \r\n0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=208 content_length=0
off=51 error code=12 reason="Invalid character in chunk size"
```

## Space after chunk header (lenient)

<!-- meta={"type": "request-lenient-spaces-after-chunk-size"} -->
```http
PUT /url HTTP/1.1
Transfer-Encoding: chunked

a \r\n0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=17 span[header_field]="Transfer-Encoding"
off=37 header_field complete
off=38 len=7 span[header_value]="chunked"
off=47 header_value complete
off=49 headers complete method=4 v=1/1 flags=208 content_length=0
off=53 chunk header len=10
off=53 len=10 span[body]="0123456789"
off=65 chunk complete
off=68 chunk header len=0
off=70 chunk complete
off=70 message complete
```




Sample requests
===============

Lots of sample requests, most ported from [http_parser][0] test suite.

## Simple request

<!-- meta={"type": "request"} -->
```http
OPTIONS /url HTTP/1.1
Header1: Value1
Header2:\t Value2


```

```log
off=0 message begin
off=0 len=7 span[method]="OPTIONS"
off=7 method complete
off=8 len=4 span[url]="/url"
off=13 url complete
off=13 len=4 span[protocol]="HTTP"
off=17 protocol complete
off=18 len=3 span[version]="1.1"
off=21 version complete
off=23 len=7 span[header_field]="Header1"
off=31 header_field complete
off=32 len=6 span[header_value]="Value1"
off=40 header_value complete
off=40 len=7 span[header_field]="Header2"
off=48 header_field complete
off=50 len=6 span[header_value]="Value2"
off=58 header_value complete
off=60 headers complete method=6 v=1/1 flags=0 content_length=0
off=60 message complete
```

## Request with method starting with `H`

There's a optimization in `start_req_or_res` that passes execution to
`start_req` when the first character is not `H` (because response must start
with `HTTP/`). However, there're still methods like `HEAD` that should get
to `start_req`. Verify that it still works after optimization.

<!-- meta={"type": "request", "noScan": true } -->
```http
HEAD /url HTTP/1.1


```

```log
off=0 message begin
off=0 len=4 span[method]="HEAD"
off=4 method complete
off=5 len=4 span[url]="/url"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.1"
off=18 version complete
off=22 headers complete method=2 v=1/1 flags=0 content_length=0
off=22 message complete
```

## curl GET

<!-- meta={"type": "request"} -->
```http
GET /test HTTP/1.1
User-Agent: curl/7.18.0 (i486-pc-linux-gnu) libcurl/7.18.0 OpenSSL/0.9.8g zlib/1.2.3.3 libidn/1.1
Host: 0.0.0.0=5000
Accept: */*


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=5 span[url]="/test"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.1"
off=18 version complete
off=20 len=10 span[header_field]="User-Agent"
off=31 header_field complete
off=32 len=85 span[header_value]="curl/7.18.0 (i486-pc-linux-gnu) libcurl/7.18.0 OpenSSL/0.9.8g zlib/1.2.3.3 libidn/1.1"
off=119 header_value complete
off=119 len=4 span[header_field]="Host"
off=124 header_field complete
off=125 len=12 span[header_value]="0.0.0.0=5000"
off=139 header_value complete
off=139 len=6 span[header_field]="Accept"
off=146 header_field complete
off=147 len=3 span[header_value]="*/*"
off=152 header_value complete
off=154 headers complete method=1 v=1/1 flags=0 content_length=0
off=154 message complete
```

## Firefox GET

<!-- meta={"type": "request"} -->
```http
GET /favicon.ico HTTP/1.1
Host: 0.0.0.0=5000
User-Agent: Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9) Gecko/2008061015 Firefox/3.0
Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8
Accept-Language: en-us,en;q=0.5
Accept-Encoding: gzip,deflate
Accept-Charset: ISO-8859-1,utf-8;q=0.7,*;q=0.7
Keep-Alive: 300
Connection: keep-alive


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=12 span[url]="/favicon.ico"
off=17 url complete
off=17 len=4 span[protocol]="HTTP"
off=21 protocol complete
off=22 len=3 span[version]="1.1"
off=25 version complete
off=27 len=4 span[header_field]="Host"
off=32 header_field complete
off=33 len=12 span[header_value]="0.0.0.0=5000"
off=47 header_value complete
off=47 len=10 span[header_field]="User-Agent"
off=58 header_field complete
off=59 len=76 span[header_value]="Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9) Gecko/2008061015 Firefox/3.0"
off=137 header_value complete
off=137 len=6 span[header_field]="Accept"
off=144 header_field complete
off=145 len=63 span[header_value]="text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
off=210 header_value complete
off=210 len=15 span[header_field]="Accept-Language"
off=226 header_field complete
off=227 len=14 span[header_value]="en-us,en;q=0.5"
off=243 header_value complete
off=243 len=15 span[header_field]="Accept-Encoding"
off=259 header_field complete
off=260 len=12 span[header_value]="gzip,deflate"
off=274 header_value complete
off=274 len=14 span[header_field]="Accept-Charset"
off=289 header_field complete
off=290 len=30 span[header_value]="ISO-8859-1,utf-8;q=0.7,*;q=0.7"
off=322 header_value complete
off=322 len=10 span[header_field]="Keep-Alive"
off=333 header_field complete
off=334 len=3 span[header_value]="300"
off=339 header_value complete
off=339 len=10 span[header_field]="Connection"
off=350 header_field complete
off=351 len=10 span[header_value]="keep-alive"
off=363 header_value complete
off=365 headers complete method=1 v=1/1 flags=1 content_length=0
off=365 message complete
```

## DUMBPACK

<!-- meta={"type": "request"} -->
```http
GET /dumbpack HTTP/1.1
aaaaaaaaaaaaa:++++++++++


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=9 span[url]="/dumbpack"
off=14 url complete
off=14 len=4 span[protocol]="HTTP"
off=18 protocol complete
off=19 len=3 span[version]="1.1"
off=22 version complete
off=24 len=13 span[header_field]="aaaaaaaaaaaaa"
off=38 header_field complete
off=38 len=10 span[header_value]="++++++++++"
off=50 header_value complete
off=52 headers complete method=1 v=1/1 flags=0 content_length=0
off=52 message complete
```

## No headers and no body

<!-- meta={"type": "request"} -->
```http
GET /get_no_headers_no_body/world HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=29 span[url]="/get_no_headers_no_body/world"
off=34 url complete
off=34 len=4 span[protocol]="HTTP"
off=38 protocol complete
off=39 len=3 span[version]="1.1"
off=42 version complete
off=46 headers complete method=1 v=1/1 flags=0 content_length=0
off=46 message complete
```

## One header and no body

<!-- meta={"type": "request"} -->
```http
GET /get_one_header_no_body HTTP/1.1
Accept: */*


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=23 span[url]="/get_one_header_no_body"
off=28 url complete
off=28 len=4 span[protocol]="HTTP"
off=32 protocol complete
off=33 len=3 span[version]="1.1"
off=36 version complete
off=38 len=6 span[header_field]="Accept"
off=45 header_field complete
off=46 len=3 span[header_value]="*/*"
off=51 header_value complete
off=53 headers complete method=1 v=1/1 flags=0 content_length=0
off=53 message complete
```

## Apache bench GET

The server receiving this request SHOULD NOT wait for EOF to know that
`Content-Length == 0`.

<!-- meta={"type": "request"} -->
```http
GET /test HTTP/1.0
Host: 0.0.0.0:5000
User-Agent: ApacheBench/2.3
Accept: */*


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=5 span[url]="/test"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.0"
off=18 version complete
off=20 len=4 span[header_field]="Host"
off=25 header_field complete
off=26 len=12 span[header_value]="0.0.0.0:5000"
off=40 header_value complete
off=40 len=10 span[header_field]="User-Agent"
off=51 header_field complete
off=52 len=15 span[header_value]="ApacheBench/2.3"
off=69 header_value complete
off=69 len=6 span[header_field]="Accept"
off=76 header_field complete
off=77 len=3 span[header_value]="*/*"
off=82 header_value complete
off=84 headers complete method=1 v=1/0 flags=0 content_length=0
off=84 message complete
```

## Prefix newline

Some clients, especially after a POST in a keep-alive connection,
will send an extra CRLF before the next request.

<!-- meta={"type": "request"} -->
```http
\r\nGET /test HTTP/1.1


```

```log
off=2 message begin
off=2 len=3 span[method]="GET"
off=5 method complete
off=6 len=5 span[url]="/test"
off=12 url complete
off=12 len=4 span[protocol]="HTTP"
off=16 protocol complete
off=17 len=3 span[version]="1.1"
off=20 version complete
off=24 headers complete method=1 v=1/1 flags=0 content_length=0
off=24 message complete
```

## No HTTP version

<!-- meta={"type": "request"} -->
```http
GET /


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=7 url complete
off=9 headers complete method=1 v=0/9 flags=0 content_length=0
off=9 message complete
```

## Line folding in header value with CRLF

<!-- meta={"type": "request-lenient-headers"} -->
```http
GET / HTTP/1.1
Line1:   abc
\tdef
 ghi
\t\tjkl
  mno 
\t \tqrs
Line2: \t line2\t
Line3:
 line3
Line4: 
 
Connection:
 close


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=5 span[header_field]="Line1"
off=22 header_field complete
off=25 len=3 span[header_value]="abc"
off=30 len=4 span[header_value]="\tdef"
off=36 len=4 span[header_value]=" ghi"
off=42 len=5 span[header_value]="\t\tjkl"
off=49 len=6 span[header_value]="  mno "
off=57 len=6 span[header_value]="\t \tqrs"
off=65 header_value complete
off=65 len=5 span[header_field]="Line2"
off=71 header_field complete
off=74 len=6 span[header_value]="line2\t"
off=82 header_value complete
off=82 len=5 span[header_field]="Line3"
off=88 header_field complete
off=91 len=5 span[header_value]="line3"
off=98 header_value complete
off=98 len=5 span[header_field]="Line4"
off=104 header_field complete
off=110 len=0 span[header_value]=""
off=110 header_value complete
off=110 len=10 span[header_field]="Connection"
off=121 header_field complete
off=124 len=5 span[header_value]="close"
off=131 header_value complete
off=133 headers complete method=1 v=1/1 flags=2 content_length=0
off=133 message complete
```

## Line folding in header value with LF

<!-- meta={"type": "request"} -->

```http
GET / HTTP/1.1
Line1:   abc\n\
\tdef\n\
 ghi\n\
\t\tjkl\n\
  mno \n\
\t \tqrs\n\
Line2: \t line2\t\n\
Line3:\n\
 line3\n\
Line4: \n\
 \n\
Connection:\n\
 close\n\
\n
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=5 span[header_field]="Line1"
off=22 header_field complete
off=25 len=3 span[header_value]="abc"
off=28 error code=25 reason="Missing expected CR after header value"
```

## No LF after CR

<!-- meta={"type":"request"} -->

```http
GET / HTTP/1.1\rLine: 1

```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=15 error code=2 reason="Expected CRLF after version"
```

## No LF after CR (lenient)

<!-- meta={"type":"request-lenient-optional-lf-after-cr"} -->

```http
GET / HTTP/1.1\rLine: 1

```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=15 len=4 span[header_field]="Line"
off=20 header_field complete
off=21 len=1 span[header_value]="1"
```

## Request starting with CRLF

<!-- meta={"type": "request"} -->
```http
\r\nGET /url HTTP/1.1
Header1: Value1


```

```log
off=2 message begin
off=2 len=3 span[method]="GET"
off=5 method complete
off=6 len=4 span[url]="/url"
off=11 url complete
off=11 len=4 span[protocol]="HTTP"
off=15 protocol complete
off=16 len=3 span[version]="1.1"
off=19 version complete
off=21 len=7 span[header_field]="Header1"
off=29 header_field complete
off=30 len=6 span[header_value]="Value1"
off=38 header_value complete
off=40 headers complete method=1 v=1/1 flags=0 content_length=0
off=40 message complete
```

## Extended Characters

See nodejs/test/parallel/test-http-headers-obstext.js

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/1.1
Test: Düsseldorf


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Test"
off=21 header_field complete
off=22 len=11 span[header_value]="Düsseldorf"
off=35 header_value complete
off=37 headers complete method=1 v=1/1 flags=0 content_length=0
off=37 message complete
```

## 255 ASCII in header value

Note: `Buffer.from([ 0xff ]).toString('latin1') === 'ÿ'`.

<!-- meta={"type": "request", "noScan": true} -->
```http
OPTIONS /url HTTP/1.1
Header1: Value1
Header2: \xffValue2


```

```log
off=0 message begin
off=0 len=7 span[method]="OPTIONS"
off=7 method complete
off=8 len=4 span[url]="/url"
off=13 url complete
off=13 len=4 span[protocol]="HTTP"
off=17 protocol complete
off=18 len=3 span[version]="1.1"
off=21 version complete
off=23 len=7 span[header_field]="Header1"
off=31 header_field complete
off=32 len=6 span[header_value]="Value1"
off=40 header_value complete
off=40 len=7 span[header_field]="Header2"
off=48 header_field complete
off=49 len=8 span[header_value]="ÿValue2"
off=59 header_value complete
off=61 headers complete method=6 v=1/1 flags=0 content_length=0
off=61 message complete
```

## X-SSL-Nonsense

See nodejs/test/parallel/test-http-headers-obstext.js

<!-- meta={"type": "request-lenient-headers"} -->
```http
GET / HTTP/1.1
X-SSL-Nonsense:   -----BEGIN CERTIFICATE-----
\tMIIFbTCCBFWgAwIBAgICH4cwDQYJKoZIhvcNAQEFBQAwcDELMAkGA1UEBhMCVUsx
\tETAPBgNVBAoTCGVTY2llbmNlMRIwEAYDVQQLEwlBdXRob3JpdHkxCzAJBgNVBAMT
\tAkNBMS0wKwYJKoZIhvcNAQkBFh5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMu
\tdWswHhcNMDYwNzI3MTQxMzI4WhcNMDcwNzI3MTQxMzI4WjBbMQswCQYDVQQGEwJV
\tSzERMA8GA1UEChMIZVNjaWVuY2UxEzARBgNVBAsTCk1hbmNoZXN0ZXIxCzAJBgNV
\tBAcTmrsogriqMWLAk1DMRcwFQYDVQQDEw5taWNoYWVsIHBhcmQYJKoZIhvcNAQEB
\tBQADggEPADCCAQoCggEBANPEQBgl1IaKdSS1TbhF3hEXSl72G9J+WC/1R64fAcEF
\tW51rEyFYiIeZGx/BVzwXbeBoNUK41OK65sxGuflMo5gLflbwJtHBRIEKAfVVp3YR
\tgW7cMA/s/XKgL1GEC7rQw8lIZT8RApukCGqOVHSi/F1SiFlPDxuDfmdiNzL31+sL
\t0iwHDdNkGjy5pyBSB8Y79dsSJtCW/iaLB0/n8Sj7HgvvZJ7x0fr+RQjYOUUfrePP
\tu2MSpFyf+9BbC/aXgaZuiCvSR+8Snv3xApQY+fULK/xY8h8Ua51iXoQ5jrgu2SqR
\twgA7BUi3G8LFzMBl8FRCDYGUDy7M6QaHXx1ZWIPWNKsCAwEAAaOCAiQwggIgMAwG
\tA1UdEwEB/wQCMAAwEQYJYIZIAYb4QgHTTPAQDAgWgMA4GA1UdDwEB/wQEAwID6DAs
\tBglghkgBhvhCAQ0EHxYdVUsgZS1TY2llbmNlIFVzZXIgQ2VydGlmaWNhdGUwHQYD
\tVR0OBBYEFDTt/sf9PeMaZDHkUIldrDYMNTBZMIGaBgNVHSMEgZIwgY+AFAI4qxGj
\tloCLDdMVKwiljjDastqooXSkcjBwMQswCQYDVQQGEwJVSzERMA8GA1UEChMIZVNj
\taWVuY2UxEjAQBgNVBAsTCUF1dGhvcml0eTELMAkGA1UEAxMCQ0ExLTArBgkqhkiG
\t9w0BCQEWHmNhLW9wZXJhdG9yQGdyaWQtc3VwcG9ydC5hYy51a4IBADApBgNVHRIE
\tIjAggR5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMudWswGQYDVR0gBBIwEDAO
\tBgwrBgEEAdkvAQEBAQYwPQYJYIZIAYb4QgEEBDAWLmh0dHA6Ly9jYS5ncmlkLXN1
\tcHBvcnQuYWMudmT4sopwqlBWsvcHViL2NybC9jYWNybC5jcmwwPQYJYIZIAYb4QgEDBDAWLmh0
\tdHA6Ly9jYS5ncmlkLXN1cHBvcnQuYWMudWsvcHViL2NybC9jYWNybC5jcmwwPwYD
\tVR0fBDgwNjA0oDKgMIYuaHR0cDovL2NhLmdyaWQt5hYy51ay9wdWIv
\tY3JsL2NhY3JsLmNybDANBgkqhkiG9w0BAQUFAAOCAQEAS/U4iiooBENGW/Hwmmd3
\tXCy6Zrt08YjKCzGNjorT98g8uGsqYjSxv/hmi0qlnlHs+k/3Iobc3LjS5AMYr5L8
\tUO7OSkgFFlLHQyC9JzPfmLCAugvzEbyv4Olnsr8hbxF1MbKZoQxUZtMVu29wjfXk
\thTeApBv7eaKCWpSp7MCbvgzm74izKhu3vlDk9w6qVrxePfGgpKPqfHiOoGhFnbTK
\twTC6o2xq5y0qZ03JonF7OJspEd3I5zKY3E+ov7/ZhW6DqT8UFvsAdjvQbXyhV8Eu
\tYhixw1aKEPzNjNowuIseVogKOLXxWI5vAi5HgXdS0/ES5gDGsABo4fqovUKlgop3
\tRA==
\t-----END CERTIFICATE-----


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=14 span[header_field]="X-SSL-Nonsense"
off=31 header_field complete
off=34 len=27 span[header_value]="-----BEGIN CERTIFICATE-----"
off=63 len=65 span[header_value]="\tMIIFbTCCBFWgAwIBAgICH4cwDQYJKoZIhvcNAQEFBQAwcDELMAkGA1UEBhMCVUsx"
off=130 len=65 span[header_value]="\tETAPBgNVBAoTCGVTY2llbmNlMRIwEAYDVQQLEwlBdXRob3JpdHkxCzAJBgNVBAMT"
off=197 len=65 span[header_value]="\tAkNBMS0wKwYJKoZIhvcNAQkBFh5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMu"
off=264 len=65 span[header_value]="\tdWswHhcNMDYwNzI3MTQxMzI4WhcNMDcwNzI3MTQxMzI4WjBbMQswCQYDVQQGEwJV"
off=331 len=65 span[header_value]="\tSzERMA8GA1UEChMIZVNjaWVuY2UxEzARBgNVBAsTCk1hbmNoZXN0ZXIxCzAJBgNV"
off=398 len=65 span[header_value]="\tBAcTmrsogriqMWLAk1DMRcwFQYDVQQDEw5taWNoYWVsIHBhcmQYJKoZIhvcNAQEB"
off=465 len=65 span[header_value]="\tBQADggEPADCCAQoCggEBANPEQBgl1IaKdSS1TbhF3hEXSl72G9J+WC/1R64fAcEF"
off=532 len=65 span[header_value]="\tW51rEyFYiIeZGx/BVzwXbeBoNUK41OK65sxGuflMo5gLflbwJtHBRIEKAfVVp3YR"
off=599 len=65 span[header_value]="\tgW7cMA/s/XKgL1GEC7rQw8lIZT8RApukCGqOVHSi/F1SiFlPDxuDfmdiNzL31+sL"
off=666 len=65 span[header_value]="\t0iwHDdNkGjy5pyBSB8Y79dsSJtCW/iaLB0/n8Sj7HgvvZJ7x0fr+RQjYOUUfrePP"
off=733 len=65 span[header_value]="\tu2MSpFyf+9BbC/aXgaZuiCvSR+8Snv3xApQY+fULK/xY8h8Ua51iXoQ5jrgu2SqR"
off=800 len=65 span[header_value]="\twgA7BUi3G8LFzMBl8FRCDYGUDy7M6QaHXx1ZWIPWNKsCAwEAAaOCAiQwggIgMAwG"
off=867 len=66 span[header_value]="\tA1UdEwEB/wQCMAAwEQYJYIZIAYb4QgHTTPAQDAgWgMA4GA1UdDwEB/wQEAwID6DAs"
off=935 len=65 span[header_value]="\tBglghkgBhvhCAQ0EHxYdVUsgZS1TY2llbmNlIFVzZXIgQ2VydGlmaWNhdGUwHQYD"
off=1002 len=65 span[header_value]="\tVR0OBBYEFDTt/sf9PeMaZDHkUIldrDYMNTBZMIGaBgNVHSMEgZIwgY+AFAI4qxGj"
off=1069 len=65 span[header_value]="\tloCLDdMVKwiljjDastqooXSkcjBwMQswCQYDVQQGEwJVSzERMA8GA1UEChMIZVNj"
off=1136 len=65 span[header_value]="\taWVuY2UxEjAQBgNVBAsTCUF1dGhvcml0eTELMAkGA1UEAxMCQ0ExLTArBgkqhkiG"
off=1203 len=65 span[header_value]="\t9w0BCQEWHmNhLW9wZXJhdG9yQGdyaWQtc3VwcG9ydC5hYy51a4IBADApBgNVHRIE"
off=1270 len=65 span[header_value]="\tIjAggR5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMudWswGQYDVR0gBBIwEDAO"
off=1337 len=65 span[header_value]="\tBgwrBgEEAdkvAQEBAQYwPQYJYIZIAYb4QgEEBDAWLmh0dHA6Ly9jYS5ncmlkLXN1"
off=1404 len=75 span[header_value]="\tcHBvcnQuYWMudmT4sopwqlBWsvcHViL2NybC9jYWNybC5jcmwwPQYJYIZIAYb4QgEDBDAWLmh0"
off=1481 len=65 span[header_value]="\tdHA6Ly9jYS5ncmlkLXN1cHBvcnQuYWMudWsvcHViL2NybC9jYWNybC5jcmwwPwYD"
off=1548 len=55 span[header_value]="\tVR0fBDgwNjA0oDKgMIYuaHR0cDovL2NhLmdyaWQt5hYy51ay9wdWIv"
off=1605 len=65 span[header_value]="\tY3JsL2NhY3JsLmNybDANBgkqhkiG9w0BAQUFAAOCAQEAS/U4iiooBENGW/Hwmmd3"
off=1672 len=65 span[header_value]="\tXCy6Zrt08YjKCzGNjorT98g8uGsqYjSxv/hmi0qlnlHs+k/3Iobc3LjS5AMYr5L8"
off=1739 len=65 span[header_value]="\tUO7OSkgFFlLHQyC9JzPfmLCAugvzEbyv4Olnsr8hbxF1MbKZoQxUZtMVu29wjfXk"
off=1806 len=65 span[header_value]="\thTeApBv7eaKCWpSp7MCbvgzm74izKhu3vlDk9w6qVrxePfGgpKPqfHiOoGhFnbTK"
off=1873 len=65 span[header_value]="\twTC6o2xq5y0qZ03JonF7OJspEd3I5zKY3E+ov7/ZhW6DqT8UFvsAdjvQbXyhV8Eu"
off=1940 len=65 span[header_value]="\tYhixw1aKEPzNjNowuIseVogKOLXxWI5vAi5HgXdS0/ES5gDGsABo4fqovUKlgop3"
off=2007 len=5 span[header_value]="\tRA=="
off=2014 len=26 span[header_value]="\t-----END CERTIFICATE-----"
off=2042 header_value complete
off=2044 headers complete method=1 v=1/1 flags=0 content_length=0
off=2044 message complete
```

[0]: https://github.com/nodejs/http-parser





Pipelining
==========

## Should parse multiple events

<!-- meta={"type": "request"} -->
```http
POST /aaa HTTP/1.1
Content-Length: 3

AAA
PUT /bbb HTTP/1.1
Content-Length: 4

BBBB
PATCH /ccc HTTP/1.1
Content-Length: 5

CCCC
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=4 span[url]="/aaa"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.1"
off=18 version complete
off=20 len=14 span[header_field]="Content-Length"
off=35 header_field complete
off=36 len=1 span[header_value]="3"
off=39 header_value complete
off=41 headers complete method=3 v=1/1 flags=20 content_length=3
off=41 len=3 span[body]="AAA"
off=44 message complete
off=46 reset
off=46 message begin
off=46 len=3 span[method]="PUT"
off=49 method complete
off=50 len=4 span[url]="/bbb"
off=55 url complete
off=55 len=4 span[protocol]="HTTP"
off=59 protocol complete
off=60 len=3 span[version]="1.1"
off=63 version complete
off=65 len=14 span[header_field]="Content-Length"
off=80 header_field complete
off=81 len=1 span[header_value]="4"
off=84 header_value complete
off=86 headers complete method=4 v=1/1 flags=20 content_length=4
off=86 len=4 span[body]="BBBB"
off=90 message complete
off=92 reset
off=92 message begin
off=92 len=5 span[method]="PATCH"
off=97 method complete
off=98 len=4 span[url]="/ccc"
off=103 url complete
off=103 len=4 span[protocol]="HTTP"
off=107 protocol complete
off=108 len=3 span[version]="1.1"
off=111 version complete
off=113 len=14 span[header_field]="Content-Length"
off=128 header_field complete
off=129 len=1 span[header_value]="5"
off=132 header_value complete
off=134 headers complete method=28 v=1/1 flags=20 content_length=5
off=134 len=4 span[body]="CCCC"
```



Pausing
=======

### on_message_begin

<!-- meta={"type": "request", "pause": "on_message_begin"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 pause
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_message_complete

<!-- meta={"type": "request", "pause": "on_message_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
off=41 pause
```

### on_protocol_complete

<!-- meta={"type": "request", "pause": "on_protocol_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=11 pause
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_method_complete

<!-- meta={"type": "request", "pause": "on_method_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=4 pause
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_url_complete

<!-- meta={"type": "request", "pause": "on_url_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 pause
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_version_complete

<!-- meta={"type": "request", "pause": "on_version_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=15 pause
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_header_field_complete

<!-- meta={"type": "request", "pause": "on_header_field_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=32 pause
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_header_value_complete

<!-- meta={"type": "request", "pause": "on_header_value_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=36 pause
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_headers_complete

<!-- meta={"type": "request", "pause": "on_headers_complete"} -->
```http
POST / HTTP/1.1
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete method=3 v=1/1 flags=20 content_length=3
off=38 pause
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_chunk_header

<!-- meta={"type": "request", "pause": "on_chunk_header"} -->
```http
PUT / HTTP/1.1
Transfer-Encoding: chunked

a
0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=17 span[header_field]="Transfer-Encoding"
off=34 header_field complete
off=35 len=7 span[header_value]="chunked"
off=44 header_value complete
off=46 headers complete method=4 v=1/1 flags=208 content_length=0
off=49 chunk header len=10
off=49 pause
off=49 len=10 span[body]="0123456789"
off=61 chunk complete
off=64 chunk header len=0
off=64 pause
off=66 chunk complete
off=66 message complete
```

### on_chunk_extension_name

<!-- meta={"type": "request", "pause": "on_chunk_extension_name"} -->
```http
PUT / HTTP/1.1
Transfer-Encoding: chunked

a;foo=bar
0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=17 span[header_field]="Transfer-Encoding"
off=34 header_field complete
off=35 len=7 span[header_value]="chunked"
off=44 header_value complete
off=46 headers complete method=4 v=1/1 flags=208 content_length=0
off=48 len=3 span[chunk_extension_name]="foo"
off=52 chunk_extension_name complete
off=52 pause
off=52 len=3 span[chunk_extension_value]="bar"
off=56 chunk_extension_value complete
off=57 chunk header len=10
off=57 len=10 span[body]="0123456789"
off=69 chunk complete
off=72 chunk header len=0
off=74 chunk complete
off=74 message complete
```

### on_chunk_extension_value

<!-- meta={"type": "request", "pause": "on_chunk_extension_value"} -->
```http
PUT / HTTP/1.1
Transfer-Encoding: chunked

a;foo=bar
0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=17 span[header_field]="Transfer-Encoding"
off=34 header_field complete
off=35 len=7 span[header_value]="chunked"
off=44 header_value complete
off=46 headers complete method=4 v=1/1 flags=208 content_length=0
off=48 len=3 span[chunk_extension_name]="foo"
off=52 chunk_extension_name complete
off=52 len=3 span[chunk_extension_value]="bar"
off=56 chunk_extension_value complete
off=56 pause
off=57 chunk header len=10
off=57 len=10 span[body]="0123456789"
off=69 chunk complete
off=72 chunk header len=0
off=74 chunk complete
off=74 message complete
```


### on_chunk_complete

<!-- meta={"type": "request", "pause": "on_chunk_complete"} -->
```http
PUT / HTTP/1.1
Transfer-Encoding: chunked

a
0123456789
0


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=17 span[header_field]="Transfer-Encoding"
off=34 header_field complete
off=35 len=7 span[header_value]="chunked"
off=44 header_value complete
off=46 headers complete method=4 v=1/1 flags=208 content_length=0
off=49 chunk header len=10
off=49 len=10 span[body]="0123456789"
off=61 chunk complete
off=61 pause
off=64 chunk header len=0
off=66 chunk complete
off=66 pause
off=66 message complete
```



Methods
=======

### REPORT request

<!-- meta={"type": "request"} -->
```http
REPORT /test HTTP/1.1


```

```log
off=0 message begin
off=0 len=6 span[method]="REPORT"
off=6 method complete
off=7 len=5 span[url]="/test"
off=13 url complete
off=13 len=4 span[protocol]="HTTP"
off=17 protocol complete
off=18 len=3 span[version]="1.1"
off=21 version complete
off=25 headers complete method=20 v=1/1 flags=0 content_length=0
off=25 message complete
```

### CONNECT request

<!-- meta={"type": "request"} -->
```http
CONNECT 0-home0.netscape.com:443 HTTP/1.0
User-agent: Mozilla/1.1N
Proxy-authorization: basic aGVsbG86d29ybGQ=

some data
and yet even more data
```

```log
off=0 message begin
off=0 len=7 span[method]="CONNECT"
off=7 method complete
off=8 len=24 span[url]="0-home0.netscape.com:443"
off=33 url complete
off=33 len=4 span[protocol]="HTTP"
off=37 protocol complete
off=38 len=3 span[version]="1.0"
off=41 version complete
off=43 len=10 span[header_field]="User-agent"
off=54 header_field complete
off=55 len=12 span[header_value]="Mozilla/1.1N"
off=69 header_value complete
off=69 len=19 span[header_field]="Proxy-authorization"
off=89 header_field complete
off=90 len=22 span[header_value]="basic aGVsbG86d29ybGQ="
off=114 header_value complete
off=116 headers complete method=5 v=1/0 flags=0 content_length=0
off=116 message complete
off=116 error code=22 reason="Pause on CONNECT/Upgrade"
```

### CONNECT request with CAPS

<!-- meta={"type": "request"} -->
```http
CONNECT HOME0.NETSCAPE.COM:443 HTTP/1.0
User-agent: Mozilla/1.1N
Proxy-authorization: basic aGVsbG86d29ybGQ=


```

```log
off=0 message begin
off=0 len=7 span[method]="CONNECT"
off=7 method complete
off=8 len=22 span[url]="HOME0.NETSCAPE.COM:443"
off=31 url complete
off=31 len=4 span[protocol]="HTTP"
off=35 protocol complete
off=36 len=3 span[version]="1.0"
off=39 version complete
off=41 len=10 span[header_field]="User-agent"
off=52 header_field complete
off=53 len=12 span[header_value]="Mozilla/1.1N"
off=67 header_value complete
off=67 len=19 span[header_field]="Proxy-authorization"
off=87 header_field complete
off=88 len=22 span[header_value]="basic aGVsbG86d29ybGQ="
off=112 header_value complete
off=114 headers complete method=5 v=1/0 flags=0 content_length=0
off=114 message complete
off=114 error code=22 reason="Pause on CONNECT/Upgrade"
```

### CONNECT with body

<!-- meta={"type": "request"} -->
```http
CONNECT foo.bar.com:443 HTTP/1.0
User-agent: Mozilla/1.1N
Proxy-authorization: basic aGVsbG86d29ybGQ=
Content-Length: 10

blarfcicle"
```

```log
off=0 message begin
off=0 len=7 span[method]="CONNECT"
off=7 method complete
off=8 len=15 span[url]="foo.bar.com:443"
off=24 url complete
off=24 len=4 span[protocol]="HTTP"
off=28 protocol complete
off=29 len=3 span[version]="1.0"
off=32 version complete
off=34 len=10 span[header_field]="User-agent"
off=45 header_field complete
off=46 len=12 span[header_value]="Mozilla/1.1N"
off=60 header_value complete
off=60 len=19 span[header_field]="Proxy-authorization"
off=80 header_field complete
off=81 len=22 span[header_value]="basic aGVsbG86d29ybGQ="
off=105 header_value complete
off=105 len=14 span[header_field]="Content-Length"
off=120 header_field complete
off=121 len=2 span[header_value]="10"
off=125 header_value complete
off=127 headers complete method=5 v=1/0 flags=20 content_length=10
off=127 message complete
off=127 error code=22 reason="Pause on CONNECT/Upgrade"
```

### M-SEARCH request

<!-- meta={"type": "request"} -->
```http
M-SEARCH * HTTP/1.1
HOST: 239.255.255.250:1900
MAN: "ssdp:discover"
ST: "ssdp:all"


```

```log
off=0 message begin
off=0 len=8 span[method]="M-SEARCH"
off=8 method complete
off=9 len=1 span[url]="*"
off=11 url complete
off=11 len=4 span[protocol]="HTTP"
off=15 protocol complete
off=16 len=3 span[version]="1.1"
off=19 version complete
off=21 len=4 span[header_field]="HOST"
off=26 header_field complete
off=27 len=20 span[header_value]="239.255.255.250:1900"
off=49 header_value complete
off=49 len=3 span[header_field]="MAN"
off=53 header_field complete
off=54 len=15 span[header_value]=""ssdp:discover""
off=71 header_value complete
off=71 len=2 span[header_field]="ST"
off=74 header_field complete
off=75 len=10 span[header_value]=""ssdp:all""
off=87 header_value complete
off=89 headers complete method=24 v=1/1 flags=0 content_length=0
off=89 message complete
```

### PATCH request

<!-- meta={"type": "request"} -->
```http
PATCH /file.txt HTTP/1.1
Host: www.example.com
Content-Type: application/example
If-Match: "e0023aa4e"
Content-Length: 10

cccccccccc
```

```log
off=0 message begin
off=0 len=5 span[method]="PATCH"
off=5 method complete
off=6 len=9 span[url]="/file.txt"
off=16 url complete
off=16 len=4 span[protocol]="HTTP"
off=20 protocol complete
off=21 len=3 span[version]="1.1"
off=24 version complete
off=26 len=4 span[header_field]="Host"
off=31 header_field complete
off=32 len=15 span[header_value]="www.example.com"
off=49 header_value complete
off=49 len=12 span[header_field]="Content-Type"
off=62 header_field complete
off=63 len=19 span[header_value]="application/example"
off=84 header_value complete
off=84 len=8 span[header_field]="If-Match"
off=93 header_field complete
off=94 len=11 span[header_value]=""e0023aa4e""
off=107 header_value complete
off=107 len=14 span[header_field]="Content-Length"
off=122 header_field complete
off=123 len=2 span[header_value]="10"
off=127 header_value complete
off=129 headers complete method=28 v=1/1 flags=20 content_length=10
off=129 len=10 span[body]="cccccccccc"
off=139 message complete
```

### PURGE request

<!-- meta={"type": "request"} -->
```http
PURGE /file.txt HTTP/1.1
Host: www.example.com


```

```log
off=0 message begin
off=0 len=5 span[method]="PURGE"
off=5 method complete
off=6 len=9 span[url]="/file.txt"
off=16 url complete
off=16 len=4 span[protocol]="HTTP"
off=20 protocol complete
off=21 len=3 span[version]="1.1"
off=24 version complete
off=26 len=4 span[header_field]="Host"
off=31 header_field complete
off=32 len=15 span[header_value]="www.example.com"
off=49 header_value complete
off=51 headers complete method=29 v=1/1 flags=0 content_length=0
off=51 message complete
```

### SEARCH request

<!-- meta={"type": "request"} -->
```http
SEARCH / HTTP/1.1
Host: www.example.com


```

```log
off=0 message begin
off=0 len=6 span[method]="SEARCH"
off=6 method complete
off=7 len=1 span[url]="/"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=4 span[header_field]="Host"
off=24 header_field complete
off=25 len=15 span[header_value]="www.example.com"
off=42 header_value complete
off=44 headers complete method=14 v=1/1 flags=0 content_length=0
off=44 message complete
```

### LINK request

<!-- meta={"type": "request"} -->
```http
LINK /images/my_dog.jpg HTTP/1.1
Host: example.com
Link: <http://example.com/profiles/joe>; rel="tag"
Link: <http://example.com/profiles/sally>; rel="tag"


```

```log
off=0 message begin
off=0 len=4 span[method]="LINK"
off=4 method complete
off=5 len=18 span[url]="/images/my_dog.jpg"
off=24 url complete
off=24 len=4 span[protocol]="HTTP"
off=28 protocol complete
off=29 len=3 span[version]="1.1"
off=32 version complete
off=34 len=4 span[header_field]="Host"
off=39 header_field complete
off=40 len=11 span[header_value]="example.com"
off=53 header_value complete
off=53 len=4 span[header_field]="Link"
off=58 header_field complete
off=59 len=44 span[header_value]="<http://example.com/profiles/joe>; rel="tag""
off=105 header_value complete
off=105 len=4 span[header_field]="Link"
off=110 header_field complete
off=111 len=46 span[header_value]="<http://example.com/profiles/sally>; rel="tag""
off=159 header_value complete
off=161 headers complete method=31 v=1/1 flags=0 content_length=0
off=161 message complete
```

### LINK request

<!-- meta={"type": "request"} -->
```http
UNLINK /images/my_dog.jpg HTTP/1.1
Host: example.com
Link: <http://example.com/profiles/sally>; rel="tag"


```

```log
off=0 message begin
off=0 len=6 span[method]="UNLINK"
off=6 method complete
off=7 len=18 span[url]="/images/my_dog.jpg"
off=26 url complete
off=26 len=4 span[protocol]="HTTP"
off=30 protocol complete
off=31 len=3 span[version]="1.1"
off=34 version complete
off=36 len=4 span[header_field]="Host"
off=41 header_field complete
off=42 len=11 span[header_value]="example.com"
off=55 header_value complete
off=55 len=4 span[header_field]="Link"
off=60 header_field complete
off=61 len=46 span[header_value]="<http://example.com/profiles/sally>; rel="tag""
off=109 header_value complete
off=111 headers complete method=32 v=1/1 flags=0 content_length=0
off=111 message complete
```

### SOURCE request

<!-- meta={"type": "request"} -->
```http
SOURCE /music/sweet/music HTTP/1.1
Host: example.com


```

```log
off=0 message begin
off=0 len=6 span[method]="SOURCE"
off=6 method complete
off=7 len=18 span[url]="/music/sweet/music"
off=26 url complete
off=26 len=4 span[protocol]="HTTP"
off=30 protocol complete
off=31 len=3 span[version]="1.1"
off=34 version complete
off=36 len=4 span[header_field]="Host"
off=41 header_field complete
off=42 len=11 span[header_value]="example.com"
off=55 header_value complete
off=57 headers complete method=33 v=1/1 flags=0 content_length=0
off=57 message complete
```

### SOURCE request with ICE

<!-- meta={"type": "request"} -->
```http
SOURCE /music/sweet/music ICE/1.0
Host: example.com


```

```log
off=0 message begin
off=0 len=6 span[method]="SOURCE"
off=6 method complete
off=7 len=18 span[url]="/music/sweet/music"
off=26 url complete
off=26 len=3 span[protocol]="ICE"
off=29 protocol complete
off=30 len=3 span[version]="1.0"
off=33 version complete
off=35 len=4 span[header_field]="Host"
off=40 header_field complete
off=41 len=11 span[header_value]="example.com"
off=54 header_value complete
off=56 headers complete method=33 v=1/0 flags=0 content_length=0
off=56 message complete
```

### OPTIONS request with RTSP

NOTE: `OPTIONS` is a valid HTTP metho too.

<!-- meta={"type": "request"} -->
```http
OPTIONS /music/sweet/music RTSP/1.0
Host: example.com


```

```log
off=0 message begin
off=0 len=7 span[method]="OPTIONS"
off=7 method complete
off=8 len=18 span[url]="/music/sweet/music"
off=27 url complete
off=27 len=4 span[protocol]="RTSP"
off=31 protocol complete
off=32 len=3 span[version]="1.0"
off=35 version complete
off=37 len=4 span[header_field]="Host"
off=42 header_field complete
off=43 len=11 span[header_value]="example.com"
off=56 header_value complete
off=58 headers complete method=6 v=1/0 flags=0 content_length=0
off=58 message complete
```

### ANNOUNCE request with RTSP

<!-- meta={"type": "request"} -->
```http
ANNOUNCE /music/sweet/music RTSP/1.0
Host: example.com


```

```log
off=0 message begin
off=0 len=8 span[method]="ANNOUNCE"
off=8 method complete
off=9 len=18 span[url]="/music/sweet/music"
off=28 url complete
off=28 len=4 span[protocol]="RTSP"
off=32 protocol complete
off=33 len=3 span[version]="1.0"
off=36 version complete
off=38 len=4 span[header_field]="Host"
off=43 header_field complete
off=44 len=11 span[header_value]="example.com"
off=57 header_value complete
off=59 headers complete method=36 v=1/0 flags=0 content_length=0
off=59 message complete
```

### PRI request HTTP2

<!-- meta={"type": "request"} -->
```http
PRI * HTTP/1.1

SM


```

```log
off=0 message begin
off=0 len=3 span[method]="PRI"
off=3 method complete
off=4 len=1 span[url]="*"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=24 error code=23 reason="Pause on PRI/Upgrade"
```

### QUERY request

<!-- meta={"type": "request"} -->
```http
QUERY /contacts HTTP/1.1
Host: example.org
Content-Type: example/query
Accept: text/csv
Content-Length: 41

select surname, givenname, email limit 10
```

```log
off=0 message begin
off=0 len=5 span[method]="QUERY"
off=5 method complete
off=6 len=9 span[url]="/contacts"
off=16 url complete
off=16 len=4 span[protocol]="HTTP"
off=20 protocol complete
off=21 len=3 span[version]="1.1"
off=24 version complete
off=26 len=4 span[header_field]="Host"
off=31 header_field complete
off=32 len=11 span[header_value]="example.org"
off=45 header_value complete
off=45 len=12 span[header_field]="Content-Type"
off=58 header_field complete
off=59 len=13 span[header_value]="example/query"
off=74 header_value complete
off=74 len=6 span[header_field]="Accept"
off=81 header_field complete
off=82 len=8 span[header_value]="text/csv"
off=92 header_value complete
off=92 len=14 span[header_field]="Content-Length"
off=107 header_field complete
off=108 len=2 span[header_value]="41"
off=112 header_value complete
off=114 headers complete method=46 v=1/1 flags=20 content_length=41
off=114 len=41 span[body]="select surname, givenname, email limit 10"
off=155 message complete
```



Lenient HTTP version parsing
============================

### Invalid HTTP version (lenient)

<!-- meta={"type": "request-lenient-version"} -->
```http
GET / HTTP/5.6


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="5.6"
off=14 version complete
off=18 headers complete method=1 v=5/6 flags=0 content_length=0
off=18 message complete
```




Invalid requests
================

### ICE protocol and GET method

<!-- meta={"type": "request"} -->
```http
GET /music/sweet/music ICE/1.0
Host: example.com


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=18 span[url]="/music/sweet/music"
off=23 url complete
off=23 len=3 span[protocol]="ICE"
off=26 protocol complete
off=26 error code=8 reason="Expected SOURCE method for ICE/x.x request"
```

### ICE protocol, but not really

<!-- meta={"type": "request"} -->
```http
GET /music/sweet/music IHTTP/1.0
Host: example.com


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=18 span[url]="/music/sweet/music"
off=23 url complete
off=23 len=1 span[protocol]="I"
off=24 error code=8 reason="Expected HTTP/, RTSP/ or ICE/"
```

### RTSP protocol and PUT method

<!-- meta={"type": "request"} -->
```http
PUT /music/sweet/music RTSP/1.0
Host: example.com


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=18 span[url]="/music/sweet/music"
off=23 url complete
off=23 len=4 span[protocol]="RTSP"
off=27 protocol complete
off=27 error code=8 reason="Invalid method for RTSP/x.x request"
```

### HTTP protocol and ANNOUNCE method

<!-- meta={"type": "request"} -->
```http
ANNOUNCE /music/sweet/music HTTP/1.0
Host: example.com


```

```log
off=0 message begin
off=0 len=8 span[method]="ANNOUNCE"
off=8 method complete
off=9 len=18 span[url]="/music/sweet/music"
off=28 url complete
off=28 len=4 span[protocol]="HTTP"
off=32 protocol complete
off=32 error code=8 reason="Invalid method for HTTP/x.x request"
```

### Headers separated by CR

<!-- meta={"type": "request"} -->
```http
GET / HTTP/1.1
Foo: 1\rBar: 2


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=3 span[header_field]="Foo"
off=20 header_field complete
off=21 len=1 span[header_value]="1"
off=23 error code=3 reason="Missing expected LF after header value"
```

### Headers separated by LF

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Host: localhost:5000
x:x\nTransfer-Encoding: chunked

1
A
0

```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=14 span[header_value]="localhost:5000"
off=39 header_value complete
off=39 len=1 span[header_field]="x"
off=41 header_field complete
off=41 len=1 span[header_value]="x"
off=42 error code=25 reason="Missing expected CR after header value"
```

### Headers separated by dummy characters

<!-- meta={"type": "request"} -->
```http
GET / HTTP/1.1
Connection: close
Host: a
\rZGET /evil: HTTP/1.1
Host: a

```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=10 span[header_field]="Connection"
off=27 header_field complete
off=28 len=5 span[header_value]="close"
off=35 header_value complete
off=35 len=4 span[header_field]="Host"
off=40 header_field complete
off=41 len=1 span[header_value]="a"
off=44 header_value complete
off=45 error code=2 reason="Expected LF after headers"
```


### Headers separated by dummy characters (lenient)

<!-- meta={"type": "request-lenient-optional-lf-after-cr"} -->
```http
GET / HTTP/1.1
Connection: close
Host: a
\rZGET /evil: HTTP/1.1
Host: a

```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=10 span[header_field]="Connection"
off=27 header_field complete
off=28 len=5 span[header_value]="close"
off=35 header_value complete
off=35 len=4 span[header_field]="Host"
off=40 header_field complete
off=41 len=1 span[header_value]="a"
off=44 header_value complete
off=45 headers complete method=1 v=1/1 flags=2 content_length=0
off=45 message complete
off=46 error code=5 reason="Data after `Connection: close`"
```

### Empty headers separated by CR

<!-- meta={"type": "request" } -->
```http
POST / HTTP/1.1
Connection: Close
Host: localhost:5000
x:\rTransfer-Encoding: chunked

1
A
0

```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=10 span[header_field]="Connection"
off=28 header_field complete
off=29 len=5 span[header_value]="Close"
off=36 header_value complete
off=36 len=4 span[header_field]="Host"
off=41 header_field complete
off=42 len=14 span[header_value]="localhost:5000"
off=58 header_value complete
off=58 len=1 span[header_field]="x"
off=60 header_field complete
off=61 error code=2 reason="Expected LF after CR"
```

### Empty headers separated by LF

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Host: localhost:5000
x:\nTransfer-Encoding: chunked

1
A
0

```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=14 span[header_value]="localhost:5000"
off=39 header_value complete
off=39 len=1 span[header_field]="x"
off=41 header_field complete
off=42 error code=10 reason="Invalid header value char"
```

### Invalid header token #1

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/1.1
Fo@: Failure


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=18 error code=10 reason="Invalid header token"
```

### Invalid header token #2

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/1.1
Foo\01\test: Bar


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=19 error code=10 reason="Invalid header token"
```

### Invalid header token #3

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/1.1
: Bar


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 error code=10 reason="Invalid header token"
```

### Invalid method

<!-- meta={"type": "request"} -->
```http
MKCOLA / HTTP/1.1


```

```log
off=0 message begin
off=0 len=5 span[method]="MKCOL"
off=5 method complete
off=5 error code=6 reason="Expected space after method"
```

### Illegal header field name line folding

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/1.1
name
 : value


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=20 error code=10 reason="Invalid header token"
```

### Corrupted Connection header

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/1.1
Host: www.example.com
Connection\r\033\065\325eep-Alive
Accept-Encoding: gzip


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Host"
off=21 header_field complete
off=22 len=15 span[header_value]="www.example.com"
off=39 header_value complete
off=49 error code=10 reason="Invalid header token"
```

### Corrupted header name

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/1.1
Host: www.example.com
X-Some-Header\r\033\065\325eep-Alive
Accept-Encoding: gzip


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Host"
off=21 header_field complete
off=22 len=15 span[header_value]="www.example.com"
off=39 header_value complete
off=52 error code=10 reason="Invalid header token"
```

### Missing CR between headers

<!-- meta={"type": "request", "noScan": true} -->
 
```http
GET / HTTP/1.1
Host: localhost
Dummy: x\nContent-Length: 23

GET / HTTP/1.1
Dummy: GET /admin HTTP/1.1
Host: localhost


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=4 span[header_field]="Host"
off=21 header_field complete
off=22 len=9 span[header_value]="localhost"
off=33 header_value complete
off=33 len=5 span[header_field]="Dummy"
off=39 header_field complete
off=40 len=1 span[header_value]="x"
off=41 error code=25 reason="Missing expected CR after header value"
```

### Invalid HTTP version

<!-- meta={"type": "request", "noScan": true} -->
```http
GET / HTTP/5.6
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="5.6"
off=14 error code=9 reason="Invalid HTTP version"
```

## Invalid space after start line

<!-- meta={"type": "request"} -->
```http
GET / HTTP/1.1
 Host: foo
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=17 error code=30 reason="Unexpected space after start line"
```


### Only LFs present

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1\n\
Transfer-Encoding: chunked\n\
Trailer: Baz
Foo: abc\n\
Bar: def\n\
\n\
1\n\
A\n\
1;abc\n\
B\n\
1;def=ghi\n\
C\n\
1;jkl="mno"\n\
D\n\
0\n\
\n\
Baz: ghi\n\
\n\
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=16 error code=9 reason="Expected CRLF after version"
```

### Only LFs present (lenient)

<!-- meta={"type": "request-lenient-all"} -->
```http
POST / HTTP/1.1\n\
Transfer-Encoding: chunked\n\
Trailer: Baz
Foo: abc\n\
Bar: def\n\
\n\
1\n\
A\n\
1;abc\n\
B\n\
1;def=ghi\n\
C\n\
1;jkl="mno"\n\
D\n\
0\n\
\n\
Baz: ghi\n\
\n
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=16 len=17 span[header_field]="Transfer-Encoding"
off=34 header_field complete
off=35 len=7 span[header_value]="chunked"
off=43 header_value complete
off=43 len=7 span[header_field]="Trailer"
off=51 header_field complete
off=52 len=3 span[header_value]="Baz"
off=57 header_value complete
off=57 len=3 span[header_field]="Foo"
off=61 header_field complete
off=62 len=3 span[header_value]="abc"
off=66 header_value complete
off=66 len=3 span[header_field]="Bar"
off=70 header_field complete
off=71 len=3 span[header_value]="def"
off=75 header_value complete
off=76 headers complete method=3 v=1/1 flags=208 content_length=0
off=78 chunk header len=1
off=78 len=1 span[body]="A"
off=80 chunk complete
off=82 len=3 span[chunk_extension_name]="abc"
off=85 chunk_extension_name complete
off=86 chunk header len=1
off=86 len=1 span[body]="B"
off=88 chunk complete
off=90 len=3 span[chunk_extension_name]="def"
off=94 chunk_extension_name complete
off=94 len=3 span[chunk_extension_value]="ghi"
off=97 chunk_extension_value complete
off=98 chunk header len=1
off=98 len=1 span[body]="C"
off=100 chunk complete
off=102 len=3 span[chunk_extension_name]="jkl"
off=106 chunk_extension_name complete
off=106 len=5 span[chunk_extension_value]=""mno""
off=111 chunk_extension_value complete
off=112 chunk header len=1
off=112 len=1 span[body]="D"
off=114 chunk complete
off=117 chunk header len=0
off=117 len=3 span[header_field]="Baz"
off=121 header_field complete
off=122 len=3 span[header_value]="ghi"
off=126 header_value complete
off=127 chunk complete
off=127 message complete
```

### Spaces before headers

<!-- meta={ "type": "request" } -->

```http
POST /hello HTTP/1.1
Host: localhost
Foo: bar
 Content-Length: 38

GET /bye HTTP/1.1
Host: localhost


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=6 span[url]="/hello"
off=12 url complete
off=12 len=4 span[protocol]="HTTP"
off=16 protocol complete
off=17 len=3 span[version]="1.1"
off=20 version complete
off=22 len=4 span[header_field]="Host"
off=27 header_field complete
off=28 len=9 span[header_value]="localhost"
off=39 header_value complete
off=39 len=3 span[header_field]="Foo"
off=43 header_field complete
off=44 len=3 span[header_value]="bar"
off=49 error code=10 reason="Unexpected whitespace after header value"
```

### Spaces before headers (lenient)

<!-- meta={ "type": "request-lenient-headers" } -->

```http
POST /hello HTTP/1.1
Host: localhost
Foo: bar
 Content-Length: 38

GET /bye HTTP/1.1
Host: localhost


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=6 span[url]="/hello"
off=12 url complete
off=12 len=4 span[protocol]="HTTP"
off=16 protocol complete
off=17 len=3 span[version]="1.1"
off=20 version complete
off=22 len=4 span[header_field]="Host"
off=27 header_field complete
off=28 len=9 span[header_value]="localhost"
off=39 header_value complete
off=39 len=3 span[header_field]="Foo"
off=43 header_field complete
off=44 len=3 span[header_value]="bar"
off=49 len=19 span[header_value]=" Content-Length: 38"
off=70 header_value complete
off=72 headers complete method=3 v=1/1 flags=0 content_length=0
off=72 message complete
off=72 reset
off=72 message begin
off=72 len=3 span[method]="GET"
off=75 method complete
off=76 len=4 span[url]="/bye"
off=81 url complete
off=81 len=4 span[protocol]="HTTP"
off=85 protocol complete
off=86 len=3 span[version]="1.1"
off=89 version complete
off=91 len=4 span[header_field]="Host"
off=96 header_field complete
off=97 len=9 span[header_value]="localhost"
off=108 header_value complete
off=110 headers complete method=1 v=1/1 flags=0 content_length=0
off=110 message complete
```



Finish
======

Those tests check the return codes and the behavior of `llhttp_finish()` C API.

## It should be safe to finish after GET request

<!-- meta={"type": "request-finish"} -->
```http
GET / HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=18 headers complete method=1 v=1/1 flags=0 content_length=0
off=18 message complete
off=NULL finish=0
```

## It should be unsafe to finish after incomplete PUT request

<!-- meta={"type": "request-finish"} -->
```http
PUT / HTTP/1.1
Content-Length: 100

```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=14 span[header_field]="Content-Length"
off=31 header_field complete
off=32 len=3 span[header_value]="100"
off=NULL finish=2
```

## It should be unsafe to finish inside of the header

<!-- meta={"type": "request-finish"} -->
```http
PUT / HTTP/1.1
Content-Leng
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=1 span[url]="/"
off=6 url complete
off=6 len=4 span[protocol]="HTTP"
off=10 protocol complete
off=11 len=3 span[version]="1.1"
off=14 version complete
off=16 len=12 span[header_field]="Content-Leng"
off=NULL finish=2
```



Content-Length header
=====================

## `Content-Length` with zeroes

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Content-Length: 003

abc
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=3 span[header_value]="003"
off=40 header_value complete
off=42 headers complete method=4 v=1/1 flags=20 content_length=3
off=42 len=3 span[body]="abc"
off=45 message complete
```

## `Content-Length` with follow-up headers

The way the parser works is that special headers (like `Content-Length`) first
set `header_state` to appropriate value, and then apply custom parsing using
that value. For `Content-Length`, in particular, the `header_state` is used for
setting the flag too.

Make sure that `header_state` is reset to `0`, so that the flag won't be
attempted to set twice (and error).

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Content-Length: 003
Ohai: world

abc
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=3 span[header_value]="003"
off=40 header_value complete
off=40 len=4 span[header_field]="Ohai"
off=45 header_field complete
off=46 len=5 span[header_value]="world"
off=53 header_value complete
off=55 headers complete method=4 v=1/1 flags=20 content_length=3
off=55 len=3 span[body]="abc"
off=58 message complete
```

## Error on `Content-Length` overflow

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Content-Length: 1000000000000000000000

```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=21 span[header_value]="100000000000000000000"
off=56 error code=11 reason="Content-Length overflow"
```

## Error on duplicate `Content-Length`

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Content-Length: 1
Content-Length: 2

```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=1 span[header_value]="1"
off=38 header_value complete
off=38 len=14 span[header_field]="Content-Length"
off=53 header_field complete
off=54 error code=4 reason="Duplicate Content-Length"
```

## Error on simultaneous `Content-Length` and `Transfer-Encoding: identity`

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Content-Length: 1
Transfer-Encoding: identity


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=1 span[header_value]="1"
off=38 header_value complete
off=38 len=17 span[header_field]="Transfer-Encoding"
off=56 header_field complete
off=56 error code=15 reason="Transfer-Encoding can't be present with Content-Length"
```

## Invalid whitespace token with `Content-Length` header field

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: upgrade
Content-Length : 4
Upgrade: ws

abcdefgh
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=7 span[header_value]="upgrade"
off=40 header_value complete
off=40 len=14 span[header_field]="Content-Length"
off=55 error code=10 reason="Invalid header field char"
```

## Invalid whitespace token with `Content-Length` header field (lenient)

<!-- meta={"type": "request-lenient-headers"} -->
```http
PUT /url HTTP/1.1
Connection: upgrade
Content-Length : 4
Upgrade: ws

abcdefgh
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=7 span[header_value]="upgrade"
off=40 header_value complete
off=40 len=15 span[header_field]="Content-Length "
off=56 header_field complete
off=57 len=1 span[header_value]="4"
off=60 header_value complete
off=60 len=7 span[header_field]="Upgrade"
off=68 header_field complete
off=69 len=2 span[header_value]="ws"
off=73 header_value complete
off=75 headers complete method=4 v=1/1 flags=34 content_length=4
off=75 len=4 span[body]="abcd"
off=79 message complete
off=79 error code=22 reason="Pause on CONNECT/Upgrade"
```

## No error on simultaneous `Content-Length` and `Transfer-Encoding: identity` (lenient)

<!-- meta={"type": "request-lenient-chunked-length"} -->
```http
PUT /url HTTP/1.1
Content-Length: 1
Transfer-Encoding: identity


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=1 span[header_value]="1"
off=38 header_value complete
off=38 len=17 span[header_field]="Transfer-Encoding"
off=56 header_field complete
off=57 len=8 span[header_value]="identity"
off=67 header_value complete
off=69 headers complete method=4 v=1/1 flags=220 content_length=1
```

## Funky `Content-Length` with body

<!-- meta={"type": "request"} -->
```http
GET /get_funky_content_length_body_hello HTTP/1.0
conTENT-Length: 5

HELLO
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=36 span[url]="/get_funky_content_length_body_hello"
off=41 url complete
off=41 len=4 span[protocol]="HTTP"
off=45 protocol complete
off=46 len=3 span[version]="1.0"
off=49 version complete
off=51 len=14 span[header_field]="conTENT-Length"
off=66 header_field complete
off=67 len=1 span[header_value]="5"
off=70 header_value complete
off=72 headers complete method=1 v=1/0 flags=20 content_length=5
off=72 len=5 span[body]="HELLO"
off=77 message complete
```

## Spaces in `Content-Length` (surrounding)

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Content-Length:  42 


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=34 len=3 span[header_value]="42 "
off=39 header_value complete
off=41 headers complete method=3 v=1/1 flags=20 content_length=42
```

### Spaces in `Content-Length` #2

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Content-Length: 4 2


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=2 span[header_value]="4 "
off=35 error code=11 reason="Invalid character in Content-Length"
```

### Spaces in `Content-Length` #3

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Content-Length: 13 37


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=3 span[header_value]="13 "
off=36 error code=11 reason="Invalid character in Content-Length"
```

### Empty `Content-Length`

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Content-Length:


```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=34 error code=11 reason="Empty Content-Length"
```

## `Content-Length` with CR instead of dash

<!-- meta={"type": "request", "noScan": true} -->
```http
PUT /url HTTP/1.1
Content\rLength: 003

abc
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=26 error code=10 reason="Invalid header token"
```

## Content-Length reset when no body is received

<!-- meta={"type": "request", "skipBody": true} -->
```http
PUT /url HTTP/1.1
Content-Length: 123

POST /url HTTP/1.1
Content-Length: 456


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=3 span[header_value]="123"
off=40 header_value complete
off=42 headers complete method=4 v=1/1 flags=20 content_length=123
off=42 skip body
off=42 message complete
off=42 reset
off=42 message begin
off=42 len=4 span[method]="POST"
off=46 method complete
off=47 len=4 span[url]="/url"
off=52 url complete
off=52 len=4 span[protocol]="HTTP"
off=56 protocol complete
off=57 len=3 span[version]="1.1"
off=60 version complete
off=62 len=14 span[header_field]="Content-Length"
off=77 header_field complete
off=78 len=3 span[header_value]="456"
off=83 header_value complete
off=85 headers complete method=3 v=1/1 flags=20 content_length=456
off=85 skip body
off=85 message complete
```

## Missing CRLF-CRLF before body

<!-- meta={"type": "request" } -->
```http
PUT /url HTTP/1.1
Content-Length: 3
\rabc
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=1 span[header_value]="3"
off=38 header_value complete
off=39 error code=2 reason="Expected LF after headers"
```

## Missing CRLF-CRLF before body (lenient)

<!-- meta={"type": "request-lenient-optional-lf-after-cr" } -->
```http
PUT /url HTTP/1.1
Content-Length: 3
\rabc
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=1 span[header_value]="3"
off=38 header_value complete
off=39 headers complete method=4 v=1/1 flags=20 content_length=3
off=39 len=3 span[body]="abc"
off=42 message complete
```




Connection header
=================

## `keep-alive`

### Setting flag

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: keep-alive


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=10 span[header_value]="keep-alive"
off=43 header_value complete
off=45 headers complete method=4 v=1/1 flags=1 content_length=0
off=45 message complete
```

### Restarting when keep-alive is explicitly

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: keep-alive

PUT /url HTTP/1.1
Connection: keep-alive


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=10 span[header_value]="keep-alive"
off=43 header_value complete
off=45 headers complete method=4 v=1/1 flags=1 content_length=0
off=45 message complete
off=45 reset
off=45 message begin
off=45 len=3 span[method]="PUT"
off=48 method complete
off=49 len=4 span[url]="/url"
off=54 url complete
off=54 len=4 span[protocol]="HTTP"
off=58 protocol complete
off=59 len=3 span[version]="1.1"
off=62 version complete
off=64 len=10 span[header_field]="Connection"
off=75 header_field complete
off=76 len=10 span[header_value]="keep-alive"
off=88 header_value complete
off=90 headers complete method=4 v=1/1 flags=1 content_length=0
off=90 message complete
```

### No restart when keep-alive is off (1.0)

<!-- meta={"type": "request" } -->
```http
PUT /url HTTP/1.0

PUT /url HTTP/1.1


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.0"
off=17 version complete
off=21 headers complete method=4 v=1/0 flags=0 content_length=0
off=21 message complete
off=22 error code=5 reason="Data after `Connection: close`"
```

### Resetting flags when keep-alive is off (1.0, lenient)

Even though we allow restarts in loose mode, the flags should be still set to
`0` upon restart.

<!-- meta={"type": "request-lenient-keep-alive"} -->
```http
PUT /url HTTP/1.0
Content-Length: 0

PUT /url HTTP/1.1
Transfer-Encoding: chunked


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.0"
off=17 version complete
off=19 len=14 span[header_field]="Content-Length"
off=34 header_field complete
off=35 len=1 span[header_value]="0"
off=38 header_value complete
off=40 headers complete method=4 v=1/0 flags=20 content_length=0
off=40 message complete
off=40 reset
off=40 message begin
off=40 len=3 span[method]="PUT"
off=43 method complete
off=44 len=4 span[url]="/url"
off=49 url complete
off=49 len=4 span[protocol]="HTTP"
off=53 protocol complete
off=54 len=3 span[version]="1.1"
off=57 version complete
off=59 len=17 span[header_field]="Transfer-Encoding"
off=77 header_field complete
off=78 len=7 span[header_value]="chunked"
off=87 header_value complete
off=89 headers complete method=4 v=1/1 flags=208 content_length=0
```

### CRLF between requests, implicit `keep-alive`

<!-- meta={"type": "request"} -->
```http
POST / HTTP/1.1
Host: www.example.com
Content-Type: application/x-www-form-urlencoded
Content-Length: 4

q=42

GET / HTTP/1.1
```
_Note the trailing CRLF above_

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=15 span[header_value]="www.example.com"
off=40 header_value complete
off=40 len=12 span[header_field]="Content-Type"
off=53 header_field complete
off=54 len=33 span[header_value]="application/x-www-form-urlencoded"
off=89 header_value complete
off=89 len=14 span[header_field]="Content-Length"
off=104 header_field complete
off=105 len=1 span[header_value]="4"
off=108 header_value complete
off=110 headers complete method=3 v=1/1 flags=20 content_length=4
off=110 len=4 span[body]="q=42"
off=114 message complete
off=118 reset
off=118 message begin
off=118 len=3 span[method]="GET"
off=121 method complete
off=122 len=1 span[url]="/"
off=124 url complete
off=124 len=4 span[protocol]="HTTP"
off=128 protocol complete
off=129 len=3 span[version]="1.1"
off=132 version complete
```

### Not treating `\r` as `-`

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: keep\ralive


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=4 span[header_value]="keep"
off=36 error code=3 reason="Missing expected LF after header value"
```

## `close`

### Setting flag on `close`

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: close


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=5 span[header_value]="close"
off=38 header_value complete
off=40 headers complete method=4 v=1/1 flags=2 content_length=0
off=40 message complete
```

### CRLF between requests, explicit `close`

`close` means closed connection

<!-- meta={"type": "request" } -->
```http
POST / HTTP/1.1
Host: www.example.com
Content-Type: application/x-www-form-urlencoded
Content-Length: 4
Connection: close

q=42

GET / HTTP/1.1
```
_Note the trailing CRLF above_

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=15 span[header_value]="www.example.com"
off=40 header_value complete
off=40 len=12 span[header_field]="Content-Type"
off=53 header_field complete
off=54 len=33 span[header_value]="application/x-www-form-urlencoded"
off=89 header_value complete
off=89 len=14 span[header_field]="Content-Length"
off=104 header_field complete
off=105 len=1 span[header_value]="4"
off=108 header_value complete
off=108 len=10 span[header_field]="Connection"
off=119 header_field complete
off=120 len=5 span[header_value]="close"
off=127 header_value complete
off=129 headers complete method=3 v=1/1 flags=22 content_length=4
off=129 len=4 span[body]="q=42"
off=133 message complete
off=138 error code=5 reason="Data after `Connection: close`"
```

### CRLF between requests, explicit `close` (lenient)

Loose mode is more lenient, and allows further requests.

<!-- meta={"type": "request-lenient-keep-alive"} -->
```http
POST / HTTP/1.1
Host: www.example.com
Content-Type: application/x-www-form-urlencoded
Content-Length: 4
Connection: close

q=42

GET / HTTP/1.1
```
_Note the trailing CRLF above_

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=1 span[url]="/"
off=7 url complete
off=7 len=4 span[protocol]="HTTP"
off=11 protocol complete
off=12 len=3 span[version]="1.1"
off=15 version complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=15 span[header_value]="www.example.com"
off=40 header_value complete
off=40 len=12 span[header_field]="Content-Type"
off=53 header_field complete
off=54 len=33 span[header_value]="application/x-www-form-urlencoded"
off=89 header_value complete
off=89 len=14 span[header_field]="Content-Length"
off=104 header_field complete
off=105 len=1 span[header_value]="4"
off=108 header_value complete
off=108 len=10 span[header_field]="Connection"
off=119 header_field complete
off=120 len=5 span[header_value]="close"
off=127 header_value complete
off=129 headers complete method=3 v=1/1 flags=22 content_length=4
off=129 len=4 span[body]="q=42"
off=133 message complete
off=137 reset
off=137 message begin
off=137 len=3 span[method]="GET"
off=140 method complete
off=141 len=1 span[url]="/"
off=143 url complete
off=143 len=4 span[protocol]="HTTP"
off=147 protocol complete
off=148 len=3 span[version]="1.1"
off=151 version complete
```

## Parsing multiple tokens

### Sample

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: close, token, upgrade, token, keep-alive


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=40 span[header_value]="close, token, upgrade, token, keep-alive"
off=73 header_value complete
off=75 headers complete method=4 v=1/1 flags=7 content_length=0
off=75 message complete
```

### Multiple tokens with folding

<!-- meta={"type": "request-lenient-headers"} -->
```http
GET /demo HTTP/1.1
Host: example.com
Connection: Something,
 Upgrade, ,Keep-Alive
Sec-WebSocket-Key2: 12998 5 Y3 1  .P00
Sec-WebSocket-Protocol: sample
Upgrade: WebSocket
Sec-WebSocket-Key1: 4 @1  46546xW%0l 1 5
Origin: http://example.com

Hot diggity dogg
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=5 span[url]="/demo"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.1"
off=18 version complete
off=20 len=4 span[header_field]="Host"
off=25 header_field complete
off=26 len=11 span[header_value]="example.com"
off=39 header_value complete
off=39 len=10 span[header_field]="Connection"
off=50 header_field complete
off=51 len=10 span[header_value]="Something,"
off=63 len=21 span[header_value]=" Upgrade, ,Keep-Alive"
off=86 header_value complete
off=86 len=18 span[header_field]="Sec-WebSocket-Key2"
off=105 header_field complete
off=106 len=18 span[header_value]="12998 5 Y3 1  .P00"
off=126 header_value complete
off=126 len=22 span[header_field]="Sec-WebSocket-Protocol"
off=149 header_field complete
off=150 len=6 span[header_value]="sample"
off=158 header_value complete
off=158 len=7 span[header_field]="Upgrade"
off=166 header_field complete
off=167 len=9 span[header_value]="WebSocket"
off=178 header_value complete
off=178 len=18 span[header_field]="Sec-WebSocket-Key1"
off=197 header_field complete
off=198 len=20 span[header_value]="4 @1  46546xW%0l 1 5"
off=220 header_value complete
off=220 len=6 span[header_field]="Origin"
off=227 header_field complete
off=228 len=18 span[header_value]="http://example.com"
off=248 header_value complete
off=250 headers complete method=1 v=1/1 flags=15 content_length=0
off=250 message complete
off=250 error code=22 reason="Pause on CONNECT/Upgrade"
```

### Multiple tokens with folding and LWS

<!-- meta={"type": "request"} -->
```http
GET /demo HTTP/1.1
Connection: keep-alive, upgrade
Upgrade: WebSocket

Hot diggity dogg
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=5 span[url]="/demo"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.1"
off=18 version complete
off=20 len=10 span[header_field]="Connection"
off=31 header_field complete
off=32 len=19 span[header_value]="keep-alive, upgrade"
off=53 header_value complete
off=53 len=7 span[header_field]="Upgrade"
off=61 header_field complete
off=62 len=9 span[header_value]="WebSocket"
off=73 header_value complete
off=75 headers complete method=1 v=1/1 flags=15 content_length=0
off=75 message complete
off=75 error code=22 reason="Pause on CONNECT/Upgrade"
```

### Multiple tokens with folding, LWS, and CRLF

<!-- meta={"type": "request-lenient-headers"} -->
```http
GET /demo HTTP/1.1
Connection: keep-alive, \r\n upgrade
Upgrade: WebSocket

Hot diggity dogg
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=5 span[url]="/demo"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.1"
off=18 version complete
off=20 len=10 span[header_field]="Connection"
off=31 header_field complete
off=32 len=12 span[header_value]="keep-alive, "
off=46 len=8 span[header_value]=" upgrade"
off=56 header_value complete
off=56 len=7 span[header_field]="Upgrade"
off=64 header_field complete
off=65 len=9 span[header_value]="WebSocket"
off=76 header_value complete
off=78 headers complete method=1 v=1/1 flags=15 content_length=0
off=78 message complete
off=78 error code=22 reason="Pause on CONNECT/Upgrade"
```

### Invalid whitespace token with `Connection` header field

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection : upgrade
Content-Length: 4
Upgrade: ws

abcdefgh
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 error code=10 reason="Invalid header field char"
```

### Invalid whitespace token with `Connection` header field (lenient)

<!-- meta={"type": "request-lenient-headers"} -->
```http
PUT /url HTTP/1.1
Connection : upgrade
Content-Length: 4
Upgrade: ws

abcdefgh
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=11 span[header_field]="Connection "
off=31 header_field complete
off=32 len=7 span[header_value]="upgrade"
off=41 header_value complete
off=41 len=14 span[header_field]="Content-Length"
off=56 header_field complete
off=57 len=1 span[header_value]="4"
off=60 header_value complete
off=60 len=7 span[header_field]="Upgrade"
off=68 header_field complete
off=69 len=2 span[header_value]="ws"
off=73 header_value complete
off=75 headers complete method=4 v=1/1 flags=34 content_length=4
off=75 len=4 span[body]="abcd"
off=79 message complete
off=79 error code=22 reason="Pause on CONNECT/Upgrade"
```

## `upgrade`

### Setting a flag and pausing

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: upgrade
Upgrade: ws


```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=7 span[header_value]="upgrade"
off=40 header_value complete
off=40 len=7 span[header_field]="Upgrade"
off=48 header_field complete
off=49 len=2 span[header_value]="ws"
off=53 header_value complete
off=55 headers complete method=4 v=1/1 flags=14 content_length=0
off=55 message complete
off=55 error code=22 reason="Pause on CONNECT/Upgrade"
```

### Emitting part of body and pausing

<!-- meta={"type": "request"} -->
```http
PUT /url HTTP/1.1
Connection: upgrade
Content-Length: 4
Upgrade: ws

abcdefgh
```

```log
off=0 message begin
off=0 len=3 span[method]="PUT"
off=3 method complete
off=4 len=4 span[url]="/url"
off=9 url complete
off=9 len=4 span[protocol]="HTTP"
off=13 protocol complete
off=14 len=3 span[version]="1.1"
off=17 version complete
off=19 len=10 span[header_field]="Connection"
off=30 header_field complete
off=31 len=7 span[header_value]="upgrade"
off=40 header_value complete
off=40 len=14 span[header_field]="Content-Length"
off=55 header_field complete
off=56 len=1 span[header_value]="4"
off=59 header_value complete
off=59 len=7 span[header_field]="Upgrade"
off=67 header_field complete
off=68 len=2 span[header_value]="ws"
off=72 header_value complete
off=74 headers complete method=4 v=1/1 flags=34 content_length=4
off=74 len=4 span[body]="abcd"
off=78 message complete
off=78 error code=22 reason="Pause on CONNECT/Upgrade"
```

### Upgrade GET request

<!-- meta={"type": "request"} -->
```http
GET /demo HTTP/1.1
Host: example.com
Connection: Upgrade
Sec-WebSocket-Key2: 12998 5 Y3 1  .P00
Sec-WebSocket-Protocol: sample
Upgrade: WebSocket
Sec-WebSocket-Key1: 4 @1  46546xW%0l 1 5
Origin: http://example.com

Hot diggity dogg
```

```log
off=0 message begin
off=0 len=3 span[method]="GET"
off=3 method complete
off=4 len=5 span[url]="/demo"
off=10 url complete
off=10 len=4 span[protocol]="HTTP"
off=14 protocol complete
off=15 len=3 span[version]="1.1"
off=18 version complete
off=20 len=4 span[header_field]="Host"
off=25 header_field complete
off=26 len=11 span[header_value]="example.com"
off=39 header_value complete
off=39 len=10 span[header_field]="Connection"
off=50 header_field complete
off=51 len=7 span[header_value]="Upgrade"
off=60 header_value complete
off=60 len=18 span[header_field]="Sec-WebSocket-Key2"
off=79 header_field complete
off=80 len=18 span[header_value]="12998 5 Y3 1  .P00"
off=100 header_value complete
off=100 len=22 span[header_field]="Sec-WebSocket-Protocol"
off=123 header_field complete
off=124 len=6 span[header_value]="sample"
off=132 header_value complete
off=132 len=7 span[header_field]="Upgrade"
off=140 header_field complete
off=141 len=9 span[header_value]="WebSocket"
off=152 header_value complete
off=152 len=18 span[header_field]="Sec-WebSocket-Key1"
off=171 header_field complete
off=172 len=20 span[header_value]="4 @1  46546xW%0l 1 5"
off=194 header_value complete
off=194 len=6 span[header_field]="Origin"
off=201 header_field complete
off=202 len=18 span[header_value]="http://example.com"
off=222 header_value complete
off=224 headers complete method=1 v=1/1 flags=14 content_length=0
off=224 message complete
off=224 error code=22 reason="Pause on CONNECT/Upgrade"
```

### Upgrade POST request

<!-- meta={"type": "request"} -->
```http
POST /demo HTTP/1.1
Host: example.com
Connection: Upgrade
Upgrade: HTTP/2.0
Content-Length: 15

sweet post body\
Hot diggity dogg
```

```log
off=0 message begin
off=0 len=4 span[method]="POST"
off=4 method complete
off=5 len=5 span[url]="/demo"
off=11 url complete
off=11 len=4 span[protocol]="HTTP"
off=15 protocol complete
off=16 len=3 span[version]="1.1"
off=19 version complete
off=21 len=4 span[header_field]="Host"
off=26 header_field complete
off=27 len=11 span[header_value]="example.com"
off=40 header_value complete
off=40 len=10 span[header_field]="Connection"
off=51 header_field complete
off=52 len=7 span[header_value]="Upgrade"
off=61 header_value complete
off=61 len=7 span[header_field]="Upgrade"
off=69 header_field complete
off=70 len=8 span[header_value]="HTTP/2.0"
off=80 header_value complete
off=80 len=14 span[header_field]="Content-Length"
off=95 header_field complete
off=96 len=2 span[header_value]="15"
off=100 header_value complete
off=102 headers complete method=3 v=1/1 flags=34 content_length=15
off=102 len=15 span[body]="sweet post body"
off=117 message complete
off=117 error code=22 reason="Pause on CONNECT/Upgrade"
```


