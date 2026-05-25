Imagine we want to extract the http blocks and write a test for each header transforming the header name into a valid rust test function where the function has a variable called message that contains the body of the http code block and ensure to preserve newlines and carriage return in the text, collapsing it into a single string in double quotes. Write out the test function for me in rust. Each main header is a test module on its own, so example: The `Transfer-Encoding-Header` is a test module on it's own.

Wrap it in a root module called "http_response_compliance".

HTTP Response Compliance
==========================

Transfer-Encoding header
========================

## Trailing space on chunked body

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Content-Type: text/plain
Transfer-Encoding: chunked

25  \r\n\
This is the data in the first chunk

1C
and this is the second one

0  \r\n\


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=12 span[header_field]="Content-Type"
off=30 header_field complete
off=31 len=10 span[header_value]="text/plain"
off=43 header_value complete
off=43 len=17 span[header_field]="Transfer-Encoding"
off=61 header_field complete
off=62 len=7 span[header_value]="chunked"
off=71 header_value complete
off=73 headers complete status=200 v=1/1 flags=208 content_length=0
off=76 error code=12 reason="Invalid character in chunk size"
```

## `chunked` before other transfer-encoding

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Accept: */*
Transfer-Encoding: chunked, deflate

World
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=6 span[header_field]="Accept"
off=24 header_field complete
off=25 len=3 span[header_value]="*/*"
off=30 header_value complete
off=30 len=17 span[header_field]="Transfer-Encoding"
off=48 header_field complete
off=49 len=16 span[header_value]="chunked, deflate"
off=67 header_value complete
off=69 headers complete status=200 v=1/1 flags=200 content_length=0
off=69 len=5 span[body]="World"
```

## multiple transfer-encoding where chunked is not the last one

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Accept: */*
Transfer-Encoding: chunked
Transfer-Encoding: identity

World
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=6 span[header_field]="Accept"
off=24 header_field complete
off=25 len=3 span[header_value]="*/*"
off=30 header_value complete
off=30 len=17 span[header_field]="Transfer-Encoding"
off=48 header_field complete
off=49 len=7 span[header_value]="chunked"
off=58 header_value complete
off=58 len=17 span[header_field]="Transfer-Encoding"
off=76 header_field complete
off=77 len=8 span[header_value]="identity"
off=87 header_value complete
off=89 headers complete status=200 v=1/1 flags=200 content_length=0
off=89 len=5 span[body]="World"
```

## `chunkedchunked` transfer-encoding does not enable chunked enconding

This check that the word `chunked` repeat more than once (with or without spaces) does not mistakenly enables chunked encoding.

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Accept: */*
Transfer-Encoding: chunkedchunked

2
OK
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=6 span[header_field]="Accept"
off=24 header_field complete
off=25 len=3 span[header_value]="*/*"
off=30 header_value complete
off=30 len=17 span[header_field]="Transfer-Encoding"
off=48 header_field complete
off=49 len=14 span[header_value]="chunkedchunked"
off=65 header_value complete
off=67 headers complete status=200 v=1/1 flags=200 content_length=0
off=67 len=1 span[body]="2"
off=68 len=1 span[body]=cr
off=69 len=1 span[body]=lf
off=70 len=2 span[body]="OK"
off=72 len=1 span[body]=cr
off=73 len=1 span[body]=lf
off=74 len=1 span[body]="0"
off=75 len=1 span[body]=cr
off=76 len=1 span[body]=lf
off=77 len=1 span[body]=cr
off=78 len=1 span[body]=lf
```

## Chunk extensions

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Host: localhost
Transfer-encoding: chunked

5;ilovew3;somuchlove=aretheseparametersfor
hello
6;blahblah;blah
 world
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=9 span[header_value]="localhost"
off=34 header_value complete
off=34 len=17 span[header_field]="Transfer-encoding"
off=52 header_field complete
off=53 len=7 span[header_value]="chunked"
off=62 header_value complete
off=64 headers complete status=200 v=1/1 flags=208 content_length=0
off=66 len=7 span[chunk_extension_name]="ilovew3"
off=74 chunk_extension_name complete
off=74 len=10 span[chunk_extension_name]="somuchlove"
off=85 chunk_extension_name complete
off=85 len=21 span[chunk_extension_value]="aretheseparametersfor"
off=107 chunk_extension_value complete
off=108 chunk header len=5
off=108 len=5 span[body]="hello"
off=115 chunk complete
off=117 len=8 span[chunk_extension_name]="blahblah"
off=126 chunk_extension_name complete
off=126 len=4 span[chunk_extension_name]="blah"
off=131 chunk_extension_name complete
off=132 chunk header len=6
off=132 len=6 span[body]=" world"
off=140 chunk complete
off=143 chunk header len=0
off=145 chunk complete
off=145 message complete
```

## No semicolon before chunk extensions

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Host: localhost
Transfer-encoding: chunked

2 erfrferferf
aa
0 rrrr


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=9 span[header_value]="localhost"
off=34 header_value complete
off=34 len=17 span[header_field]="Transfer-encoding"
off=52 header_field complete
off=53 len=7 span[header_value]="chunked"
off=62 header_value complete
off=64 headers complete status=200 v=1/1 flags=208 content_length=0
off=66 error code=12 reason="Invalid character in chunk size"
```


## No extension after semicolon

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Host: localhost
Transfer-encoding: chunked

2;
aa
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=9 span[header_value]="localhost"
off=34 header_value complete
off=34 len=17 span[header_field]="Transfer-encoding"
off=52 header_field complete
off=53 len=7 span[header_value]="chunked"
off=62 header_value complete
off=64 headers complete status=200 v=1/1 flags=208 content_length=0
off=67 error code=2 reason="Invalid character in chunk extensions"
```


## Chunk extensions quoting

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Host: localhost
Transfer-Encoding: chunked

5;ilovew3="I love; extensions";somuchlove="aretheseparametersfor";blah;foo=bar
hello
6;blahblah;blah
 world
0

```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=9 span[header_value]="localhost"
off=34 header_value complete
off=34 len=17 span[header_field]="Transfer-Encoding"
off=52 header_field complete
off=53 len=7 span[header_value]="chunked"
off=62 header_value complete
off=64 headers complete status=200 v=1/1 flags=208 content_length=0
off=66 len=7 span[chunk_extension_name]="ilovew3"
off=74 chunk_extension_name complete
off=74 len=20 span[chunk_extension_value]=""I love; extensions""
off=94 chunk_extension_value complete
off=95 len=10 span[chunk_extension_name]="somuchlove"
off=106 chunk_extension_name complete
off=106 len=23 span[chunk_extension_value]=""aretheseparametersfor""
off=129 chunk_extension_value complete
off=130 len=4 span[chunk_extension_name]="blah"
off=135 chunk_extension_name complete
off=135 len=3 span[chunk_extension_name]="foo"
off=139 chunk_extension_name complete
off=139 len=3 span[chunk_extension_value]="bar"
off=143 chunk_extension_value complete
off=144 chunk header len=5
off=144 len=5 span[body]="hello"
off=151 chunk complete
off=153 len=8 span[chunk_extension_name]="blahblah"
off=162 chunk_extension_name complete
off=162 len=4 span[chunk_extension_name]="blah"
off=167 chunk_extension_name complete
off=168 chunk header len=6
off=168 len=6 span[body]=" world"
off=176 chunk complete
off=179 chunk header len=0
```


## Unbalanced chunk extensions quoting

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Host: localhost
Transfer-Encoding: chunked

5;ilovew3="abc";somuchlove="def; ghi
hello
6;blahblah;blah
 world
0

```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=4 span[header_field]="Host"
off=22 header_field complete
off=23 len=9 span[header_value]="localhost"
off=34 header_value complete
off=34 len=17 span[header_field]="Transfer-Encoding"
off=52 header_field complete
off=53 len=7 span[header_value]="chunked"
off=62 header_value complete
off=64 headers complete status=200 v=1/1 flags=208 content_length=0
off=66 len=7 span[chunk_extension_name]="ilovew3"
off=74 chunk_extension_name complete
off=74 len=5 span[chunk_extension_value]=""abc""
off=79 chunk_extension_value complete
off=80 len=10 span[chunk_extension_name]="somuchlove"
off=91 chunk_extension_name complete
off=91 len=9 span[chunk_extension_value]=""def; ghi"
off=101 error code=2 reason="Invalid character in chunk extensions quoted value"
```


## Invalid OBS fold after chunked value

<!-- meta={"type": "response-lenient-headers" } -->
```http
HTTP/1.1 200 OK
Transfer-Encoding: chunked
  abc

5
World
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=17 span[header_field]="Transfer-Encoding"
off=35 header_field complete
off=36 len=7 span[header_value]="chunked"
off=45 len=5 span[header_value]="  abc"
off=52 header_value complete
off=54 headers complete status=200 v=1/1 flags=200 content_length=0
off=54 len=1 span[body]="5"
off=55 len=1 span[body]=cr
off=56 len=1 span[body]=lf
off=57 len=5 span[body]="World"
off=62 len=1 span[body]=cr
off=63 len=1 span[body]=lf
off=64 len=1 span[body]="0"
off=65 len=1 span[body]=cr
off=66 len=1 span[body]=lf
off=67 len=1 span[body]=cr
off=68 len=1 span[body]=lf
```


Sample responses
================

## Simple response

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Header1: Value1
Header2:\t Value2
Content-Length: 0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=7 span[header_field]="Header1"
off=25 header_field complete
off=26 len=6 span[header_value]="Value1"
off=34 header_value complete
off=34 len=7 span[header_field]="Header2"
off=42 header_field complete
off=44 len=6 span[header_value]="Value2"
off=52 header_value complete
off=52 len=14 span[header_field]="Content-Length"
off=67 header_field complete
off=68 len=1 span[header_value]="0"
off=71 header_value complete
off=73 headers complete status=200 v=1/1 flags=20 content_length=0
off=73 message complete
```

## RTSP response

<!-- meta={"type": "response"} -->
```http
RTSP/1.1 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="RTSP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=19 headers complete status=200 v=1/1 flags=0 content_length=0
```

## ICE response

<!-- meta={"type": "response"} -->
```http
ICE/1.1 200 OK


```

```log
off=0 message begin
off=0 len=3 span[protocol]="ICE"
off=3 protocol complete
off=4 len=3 span[version]="1.1"
off=7 version complete
off=12 len=2 span[status]="OK"
off=16 status complete
off=18 headers complete status=200 v=1/1 flags=0 content_length=0
```

## Error on invalid response start

Every response must start with `HTTP/`.

<!-- meta={"type": "response"} -->
```http
HTTPER/1.1 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=4 error code=8 reason="Expected HTTP/, RTSP/ or ICE/"
```

## Empty body should not trigger spurious span callbacks

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=19 headers complete status=200 v=1/1 flags=0 content_length=0
```

## Google 301

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 301 Moved Permanently
Location: http://www.google.com/
Content-Type: text/html; charset=UTF-8
Date: Sun, 26 Apr 2009 11:11:49 GMT
Expires: Tue, 26 May 2009 11:11:49 GMT
X-$PrototypeBI-Version: 1.6.0.3
Cache-Control: public, max-age=2592000
Server: gws
Content-Length:  219

<HTML><HEAD><meta http-equiv=content-type content=text/html;charset=utf-8>\n\
<TITLE>301 Moved</TITLE></HEAD><BODY>\n\
<H1>301 Moved</H1>\n\
The document has moved\n\
<A HREF="http://www.google.com/">here</A>.
</BODY></HTML>
```
_(Note the `$` char in header field)_

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=17 span[status]="Moved Permanently"
off=32 status complete
off=32 len=8 span[header_field]="Location"
off=41 header_field complete
off=42 len=22 span[header_value]="http://www.google.com/"
off=66 header_value complete
off=66 len=12 span[header_field]="Content-Type"
off=79 header_field complete
off=80 len=24 span[header_value]="text/html; charset=UTF-8"
off=106 header_value complete
off=106 len=4 span[header_field]="Date"
off=111 header_field complete
off=112 len=29 span[header_value]="Sun, 26 Apr 2009 11:11:49 GMT"
off=143 header_value complete
off=143 len=7 span[header_field]="Expires"
off=151 header_field complete
off=152 len=29 span[header_value]="Tue, 26 May 2009 11:11:49 GMT"
off=183 header_value complete
off=183 len=22 span[header_field]="X-$PrototypeBI-Version"
off=206 header_field complete
off=207 len=7 span[header_value]="1.6.0.3"
off=216 header_value complete
off=216 len=13 span[header_field]="Cache-Control"
off=230 header_field complete
off=231 len=23 span[header_value]="public, max-age=2592000"
off=256 header_value complete
off=256 len=6 span[header_field]="Server"
off=263 header_field complete
off=264 len=3 span[header_value]="gws"
off=269 header_value complete
off=269 len=14 span[header_field]="Content-Length"
off=284 header_field complete
off=286 len=5 span[header_value]="219  "
off=293 header_value complete
off=295 headers complete status=301 v=1/1 flags=20 content_length=219
off=295 len=74 span[body]="<HTML><HEAD><meta http-equiv=content-type content=text/html;charset=utf-8>"
off=369 len=1 span[body]=lf
off=370 len=37 span[body]="<TITLE>301 Moved</TITLE></HEAD><BODY>"
off=407 len=1 span[body]=lf
off=408 len=18 span[body]="<H1>301 Moved</H1>"
off=426 len=1 span[body]=lf
off=427 len=22 span[body]="The document has moved"
off=449 len=1 span[body]=lf
off=450 len=42 span[body]="<A HREF="http://www.google.com/">here</A>."
off=492 len=1 span[body]=cr
off=493 len=1 span[body]=lf
off=494 len=14 span[body]="</BODY></HTML>"
```

## amazon.com

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 301 MovedPermanently
Date: Wed, 15 May 2013 17:06:33 GMT
Server: Server
x-amz-id-1: 0GPHKXSJQ826RK7GZEB2
p3p: policyref="http://www.amazon.com/w3c/p3p.xml",CP="CAO DSP LAW CUR ADM IVAo IVDo CONo OTPo OUR DELi PUBi OTRi BUS PHY ONL UNI PUR FIN COM NAV INT DEM CNT STA HEA PRE LOC GOV OTC "
x-amz-id-2: STN69VZxIFSz9YJLbz1GDbxpbjG6Qjmmq5E3DxRhOUw+Et0p4hr7c/Q8qNcx4oAD
Location: http://www.amazon.com/Dan-Brown/e/B000AP9DSU/ref=s9_pop_gw_al1?_encoding=UTF8&refinementId=618073011&pf_rd_m=ATVPDKIKX0DER&pf_rd_s=center-2&pf_rd_r=0SHYY5BZXN3KR20BNFAY&pf_rd_t=101&pf_rd_p=1263340922&pf_rd_i=507846
Vary: Accept-Encoding,User-Agent
Content-Type: text/html; charset=ISO-8859-1
Transfer-Encoding: chunked

1
\n
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=16 span[status]="MovedPermanently"
off=31 status complete
off=31 len=4 span[header_field]="Date"
off=36 header_field complete
off=37 len=29 span[header_value]="Wed, 15 May 2013 17:06:33 GMT"
off=68 header_value complete
off=68 len=6 span[header_field]="Server"
off=75 header_field complete
off=76 len=6 span[header_value]="Server"
off=84 header_value complete
off=84 len=10 span[header_field]="x-amz-id-1"
off=95 header_field complete
off=96 len=20 span[header_value]="0GPHKXSJQ826RK7GZEB2"
off=118 header_value complete
off=118 len=3 span[header_field]="p3p"
off=122 header_field complete
off=123 len=178 span[header_value]="policyref="http://www.amazon.com/w3c/p3p.xml",CP="CAO DSP LAW CUR ADM IVAo IVDo CONo OTPo OUR DELi PUBi OTRi BUS PHY ONL UNI PUR FIN COM NAV INT DEM CNT STA HEA PRE LOC GOV OTC ""
off=303 header_value complete
off=303 len=10 span[header_field]="x-amz-id-2"
off=314 header_field complete
off=315 len=64 span[header_value]="STN69VZxIFSz9YJLbz1GDbxpbjG6Qjmmq5E3DxRhOUw+Et0p4hr7c/Q8qNcx4oAD"
off=381 header_value complete
off=381 len=8 span[header_field]="Location"
off=390 header_field complete
off=391 len=214 span[header_value]="http://www.amazon.com/Dan-Brown/e/B000AP9DSU/ref=s9_pop_gw_al1?_encoding=UTF8&refinementId=618073011&pf_rd_m=ATVPDKIKX0DER&pf_rd_s=center-2&pf_rd_r=0SHYY5BZXN3KR20BNFAY&pf_rd_t=101&pf_rd_p=1263340922&pf_rd_i=507846"
off=607 header_value complete
off=607 len=4 span[header_field]="Vary"
off=612 header_field complete
off=613 len=26 span[header_value]="Accept-Encoding,User-Agent"
off=641 header_value complete
off=641 len=12 span[header_field]="Content-Type"
off=654 header_field complete
off=655 len=29 span[header_value]="text/html; charset=ISO-8859-1"
off=686 header_value complete
off=686 len=17 span[header_field]="Transfer-Encoding"
off=704 header_field complete
off=705 len=7 span[header_value]="chunked"
off=714 header_value complete
off=716 headers complete status=301 v=1/1 flags=208 content_length=0
off=719 chunk header len=1
off=719 len=1 span[body]=lf
off=722 chunk complete
off=725 chunk header len=0
off=727 chunk complete
off=727 message complete
```

## No headers and no body

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 404 Not Found


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=9 span[status]="Not Found"
off=24 status complete
off=26 headers complete status=404 v=1/1 flags=0 content_length=0
```

## No reason phrase

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 301


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=14 status complete
off=16 headers complete status=301 v=1/1 flags=0 content_length=0
```

## Empty reason phrase after space

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 \r\n\


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=0 span[status]=""
off=15 status complete
off=17 headers complete status=200 v=1/1 flags=0 content_length=0
```

## No carriage ret

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK\n\
Content-Type: text/html; charset=utf-8\n\
Connection: close\n\
\n\
these headers are from http://news.ycombinator.com/
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=16 error code=25 reason="Missing expected CR after response line"
```

## No carriage ret (lenient)

<!-- meta={"type": "response-lenient-optional-cr-before-lf"} -->
```http
HTTP/1.1 200 OK\n\
Content-Type: text/html; charset=utf-8\n\
Connection: close\n\
\n\
these headers are from http://news.ycombinator.com/
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=16 status complete
off=16 len=12 span[header_field]="Content-Type"
off=29 header_field complete
off=30 len=24 span[header_value]="text/html; charset=utf-8"
off=55 header_value complete
off=55 len=10 span[header_field]="Connection"
off=66 header_field complete
off=67 len=5 span[header_value]="close"
off=73 header_value complete
off=74 headers complete status=200 v=1/1 flags=2 content_length=0
off=74 len=51 span[body]="these headers are from http://news.ycombinator.com/"
```

## Underscore in header key

from: `"http://ad.doubleclick.net/pfadx/DARTSHELLCONFIGXML;dcmt=text/xml;"`

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Server: DCLK-AdSvr
Content-Type: text/xml
Content-Length: 0
DCLK_imp: v7;x;114750856;0-0;0;17820020;0/0;21603567/21621457/1;;~okv=;dcmt=text/xml;;~cs=o


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=6 span[header_field]="Server"
off=24 header_field complete
off=25 len=10 span[header_value]="DCLK-AdSvr"
off=37 header_value complete
off=37 len=12 span[header_field]="Content-Type"
off=50 header_field complete
off=51 len=8 span[header_value]="text/xml"
off=61 header_value complete
off=61 len=14 span[header_field]="Content-Length"
off=76 header_field complete
off=77 len=1 span[header_value]="0"
off=80 header_value complete
off=80 len=8 span[header_field]="DCLK_imp"
off=89 header_field complete
off=90 len=81 span[header_value]="v7;x;114750856;0-0;0;17820020;0/0;21603567/21621457/1;;~okv=;dcmt=text/xml;;~cs=o"
off=173 header_value complete
off=175 headers complete status=200 v=1/1 flags=20 content_length=0
off=175 message complete
```

## bonjourmadame.fr

The client should not merge two headers fields when the first one doesn't
have a value.

<!-- meta={"type": "response"} -->
```http
HTTP/1.0 301 Moved Permanently
Date: Thu, 03 Jun 2010 09:56:32 GMT
Server: Apache/2.2.3 (Red Hat)
Cache-Control: public
Pragma: \r\n\
Location: http://www.bonjourmadame.fr/
Vary: Accept-Encoding
Content-Length: 0
Content-Type: text/html; charset=UTF-8
Connection: keep-alive


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.0"
off=8 version complete
off=13 len=17 span[status]="Moved Permanently"
off=32 status complete
off=32 len=4 span[header_field]="Date"
off=37 header_field complete
off=38 len=29 span[header_value]="Thu, 03 Jun 2010 09:56:32 GMT"
off=69 header_value complete
off=69 len=6 span[header_field]="Server"
off=76 header_field complete
off=77 len=22 span[header_value]="Apache/2.2.3 (Red Hat)"
off=101 header_value complete
off=101 len=13 span[header_field]="Cache-Control"
off=115 header_field complete
off=116 len=6 span[header_value]="public"
off=124 header_value complete
off=124 len=6 span[header_field]="Pragma"
off=131 header_field complete
off=134 len=0 span[header_value]=""
off=134 header_value complete
off=134 len=8 span[header_field]="Location"
off=143 header_field complete
off=144 len=28 span[header_value]="http://www.bonjourmadame.fr/"
off=174 header_value complete
off=174 len=4 span[header_field]="Vary"
off=179 header_field complete
off=180 len=15 span[header_value]="Accept-Encoding"
off=197 header_value complete
off=197 len=14 span[header_field]="Content-Length"
off=212 header_field complete
off=213 len=1 span[header_value]="0"
off=216 header_value complete
off=216 len=12 span[header_field]="Content-Type"
off=229 header_field complete
off=230 len=24 span[header_value]="text/html; charset=UTF-8"
off=256 header_value complete
off=256 len=10 span[header_field]="Connection"
off=267 header_field complete
off=268 len=10 span[header_value]="keep-alive"
off=280 header_value complete
off=282 headers complete status=301 v=1/0 flags=21 content_length=0
off=282 message complete
```

## Spaces in header value

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Date: Tue, 28 Sep 2010 01:14:13 GMT
Server: Apache
Cache-Control: no-cache, must-revalidate
Expires: Mon, 26 Jul 1997 05:00:00 GMT
.et-Cookie: PlaxoCS=1274804622353690521; path=/; domain=.plaxo.com
Vary: Accept-Encoding
_eep-Alive: timeout=45
_onnection: Keep-Alive
Transfer-Encoding: chunked
Content-Type: text/html
Connection: close

0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=4 span[header_field]="Date"
off=22 header_field complete
off=23 len=29 span[header_value]="Tue, 28 Sep 2010 01:14:13 GMT"
off=54 header_value complete
off=54 len=6 span[header_field]="Server"
off=61 header_field complete
off=62 len=6 span[header_value]="Apache"
off=70 header_value complete
off=70 len=13 span[header_field]="Cache-Control"
off=84 header_field complete
off=85 len=25 span[header_value]="no-cache, must-revalidate"
off=112 header_value complete
off=112 len=7 span[header_field]="Expires"
off=120 header_field complete
off=121 len=29 span[header_value]="Mon, 26 Jul 1997 05:00:00 GMT"
off=152 header_value complete
off=152 len=10 span[header_field]=".et-Cookie"
off=163 header_field complete
off=164 len=54 span[header_value]="PlaxoCS=1274804622353690521; path=/; domain=.plaxo.com"
off=220 header_value complete
off=220 len=4 span[header_field]="Vary"
off=225 header_field complete
off=226 len=15 span[header_value]="Accept-Encoding"
off=243 header_value complete
off=243 len=10 span[header_field]="_eep-Alive"
off=254 header_field complete
off=255 len=10 span[header_value]="timeout=45"
off=267 header_value complete
off=267 len=10 span[header_field]="_onnection"
off=278 header_field complete
off=279 len=10 span[header_value]="Keep-Alive"
off=291 header_value complete
off=291 len=17 span[header_field]="Transfer-Encoding"
off=309 header_field complete
off=310 len=7 span[header_value]="chunked"
off=319 header_value complete
off=319 len=12 span[header_field]="Content-Type"
off=332 header_field complete
off=333 len=9 span[header_value]="text/html"
off=344 header_value complete
off=344 len=10 span[header_field]="Connection"
off=355 header_field complete
off=356 len=5 span[header_value]="close"
off=363 header_value complete
off=365 headers complete status=200 v=1/1 flags=20a content_length=0
off=368 chunk header len=0
off=370 chunk complete
off=370 message complete
```

## Spaces in header name

<!-- meta={"type": "response",  "noScan": true} -->
```http
HTTP/1.1 200 OK
Server: Microsoft-IIS/6.0
X-Powered-By: ASP.NET
en-US Content-Type: text/xml
Content-Type: text/xml
Content-Length: 16
Date: Fri, 23 Jul 2010 18:45:38 GMT
Connection: keep-alive

<xml>hello</xml>
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=6 span[header_field]="Server"
off=24 header_field complete
off=25 len=17 span[header_value]="Microsoft-IIS/6.0"
off=44 header_value complete
off=44 len=12 span[header_field]="X-Powered-By"
off=57 header_field complete
off=58 len=7 span[header_value]="ASP.NET"
off=67 header_value complete
off=72 error code=10 reason="Invalid header token"
```

## Non ASCII in status line

<!-- meta={"type": "response", "noScan": true} -->
```http
HTTP/1.1 500 Oriëntatieprobleem
Date: Fri, 5 Nov 2010 23:07:12 GMT+2
Content-Length: 0
Connection: close


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=19 span[status]="Oriëntatieprobleem"
off=34 status complete
off=34 len=4 span[header_field]="Date"
off=39 header_field complete
off=40 len=30 span[header_value]="Fri, 5 Nov 2010 23:07:12 GMT+2"
off=72 header_value complete
off=72 len=14 span[header_field]="Content-Length"
off=87 header_field complete
off=88 len=1 span[header_value]="0"
off=91 header_value complete
off=91 len=10 span[header_field]="Connection"
off=102 header_field complete
off=103 len=5 span[header_value]="close"
off=110 header_value complete
off=112 headers complete status=500 v=1/1 flags=22 content_length=0
off=112 message complete
```

## HTTP version 0.9

<!-- meta={"type": "response"} -->
```http
HTTP/0.9 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="0.9"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=19 headers complete status=200 v=0/9 flags=0 content_length=0
```

## No Content-Length, no Transfer-Encoding

The client should wait for the server's EOF. That is, when neither
content-length nor transfer-encoding is specified, the end of body
is specified by the EOF.

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Content-Type: text/plain

hello world
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=12 span[header_field]="Content-Type"
off=30 header_field complete
off=31 len=10 span[header_value]="text/plain"
off=43 header_value complete
off=45 headers complete status=200 v=1/1 flags=0 content_length=0
off=45 len=11 span[body]="hello world"
```

## Response starting with CRLF

<!-- meta={"type": "response"} -->
```http
\r\nHTTP/1.1 200 OK
Header1: Value1
Header2:\t Value2
Content-Length: 0


```

```log
off=2 message begin
off=2 len=4 span[protocol]="HTTP"
off=6 protocol complete
off=7 len=3 span[version]="1.1"
off=10 version complete
off=15 len=2 span[status]="OK"
off=19 status complete
off=19 len=7 span[header_field]="Header1"
off=27 header_field complete
off=28 len=6 span[header_value]="Value1"
off=36 header_value complete
off=36 len=7 span[header_field]="Header2"
off=44 header_field complete
off=46 len=6 span[header_value]="Value2"
off=54 header_value complete
off=54 len=14 span[header_field]="Content-Length"
off=69 header_field complete
off=70 len=1 span[header_value]="0"
off=73 header_value complete
off=75 headers complete status=200 v=1/1 flags=20 content_length=0
off=75 message complete
```

Pipelining
==========

## Should parse multiple events

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

AAA
HTTP/1.1 201 Created
Content-Length: 4

BBBB
HTTP/1.1 202 Accepted
Content-Length: 5

CCCC
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="AAA"
off=41 message complete
off=43 reset
off=43 message begin
off=43 len=4 span[protocol]="HTTP"
off=47 protocol complete
off=48 len=3 span[version]="1.1"
off=51 version complete
off=56 len=7 span[status]="Created"
off=65 status complete
off=65 len=14 span[header_field]="Content-Length"
off=80 header_field complete
off=81 len=1 span[header_value]="4"
off=84 header_value complete
off=86 headers complete status=201 v=1/1 flags=20 content_length=4
off=86 len=4 span[body]="BBBB"
off=90 message complete
off=92 reset
off=92 message begin
off=92 len=4 span[protocol]="HTTP"
off=96 protocol complete
off=97 len=3 span[version]="1.1"
off=100 version complete
off=105 len=8 span[status]="Accepted"
off=115 status complete
off=115 len=14 span[header_field]="Content-Length"
off=130 header_field complete
off=131 len=1 span[header_value]="5"
off=134 header_value complete
off=136 headers complete status=202 v=1/1 flags=20 content_length=5
off=136 len=4 span[body]="CCCC"
```

Pausing
=======

### on_message_begin

<!-- meta={"type": "response", "pause": "on_message_begin"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 pause
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_message_complete

<!-- meta={"type": "response", "pause": "on_message_complete"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
off=41 pause
```

### on_version_complete

<!-- meta={"type": "response", "pause": "on_version_complete"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=8 pause
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_status_complete

<!-- meta={"type": "response", "pause": "on_status_complete"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 pause
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_header_field_complete

<!-- meta={"type": "response", "pause": "on_header_field_complete"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=32 pause
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_header_value_complete

<!-- meta={"type": "response", "pause": "on_header_value_complete"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=36 pause
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_headers_complete

<!-- meta={"type": "response", "pause": "on_headers_complete"} -->
```http
HTTP/1.1 200 OK
Content-Length: 3

abc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=1 span[header_value]="3"
off=36 header_value complete
off=38 headers complete status=200 v=1/1 flags=20 content_length=3
off=38 pause
off=38 len=3 span[body]="abc"
off=41 message complete
```

### on_chunk_header

<!-- meta={"type": "response", "pause": "on_chunk_header"} -->
```http
HTTP/1.1 200 OK
Transfer-Encoding: chunked

a
0123456789
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=17 span[header_field]="Transfer-Encoding"
off=35 header_field complete
off=36 len=7 span[header_value]="chunked"
off=45 header_value complete
off=47 headers complete status=200 v=1/1 flags=208 content_length=0
off=50 chunk header len=10
off=50 pause
off=50 len=10 span[body]="0123456789"
off=62 chunk complete
off=65 chunk header len=0
off=65 pause
off=67 chunk complete
off=67 message complete
```

### on_chunk_extension_name

<!-- meta={"type": "response", "pause": "on_chunk_extension_name"} -->
```http
HTTP/1.1 200 OK
Transfer-Encoding: chunked

a;foo=bar
0123456789
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=17 span[header_field]="Transfer-Encoding"
off=35 header_field complete
off=36 len=7 span[header_value]="chunked"
off=45 header_value complete
off=47 headers complete status=200 v=1/1 flags=208 content_length=0
off=49 len=3 span[chunk_extension_name]="foo"
off=53 chunk_extension_name complete
off=53 pause
off=53 len=3 span[chunk_extension_value]="bar"
off=57 chunk_extension_value complete
off=58 chunk header len=10
off=58 len=10 span[body]="0123456789"
off=70 chunk complete
off=73 chunk header len=0
off=75 chunk complete
off=75 message complete
```

### on_chunk_extension_value

<!-- meta={"type": "response", "pause": "on_chunk_extension_value"} -->
```http
HTTP/1.1 200 OK
Transfer-Encoding: chunked

a;foo=bar
0123456789
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=17 span[header_field]="Transfer-Encoding"
off=35 header_field complete
off=36 len=7 span[header_value]="chunked"
off=45 header_value complete
off=47 headers complete status=200 v=1/1 flags=208 content_length=0
off=49 len=3 span[chunk_extension_name]="foo"
off=53 chunk_extension_name complete
off=53 len=3 span[chunk_extension_value]="bar"
off=57 chunk_extension_value complete
off=57 pause
off=58 chunk header len=10
off=58 len=10 span[body]="0123456789"
off=70 chunk complete
off=73 chunk header len=0
off=75 chunk complete
off=75 message complete
```

### on_chunk_complete

<!-- meta={"type": "response", "pause": "on_chunk_complete"} -->
```http
HTTP/1.1 200 OK
Transfer-Encoding: chunked

a
0123456789
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=17 span[header_field]="Transfer-Encoding"
off=35 header_field complete
off=36 len=7 span[header_value]="chunked"
off=45 header_value complete
off=47 headers complete status=200 v=1/1 flags=208 content_length=0
off=50 chunk header len=10
off=50 len=10 span[body]="0123456789"
off=62 chunk complete
off=62 pause
off=65 chunk header len=0
off=67 chunk complete
off=67 pause
off=67 message complete
```

Lenient HTTP version parsing
============================

### Invalid HTTP version (lenient)

<!-- meta={"type": "response-lenient-version"} -->
```http
HTTP/5.6 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="5.6"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=19 headers complete status=200 v=5/6 flags=0 content_length=0
```

Invalid responses
=================

### Incomplete HTTP protocol

<!-- meta={"type": "response"} -->
```http
HTP/1.1 200 OK


```

```log
off=0 message begin
off=0 len=2 span[protocol]="HT"
off=2 error code=8 reason="Expected HTTP/, RTSP/ or ICE/"
```

### Extra digit in HTTP major version

<!-- meta={"type": "response"} -->
```http
HTTP/01.1 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=1 span[version]="0"
off=6 error code=9 reason="Expected dot"
```

### Extra digit in HTTP major version #2

<!-- meta={"type": "response"} -->
```http
HTTP/11.1 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=1 span[version]="1"
off=6 error code=9 reason="Expected dot"
```

### Extra digit in HTTP minor version

<!-- meta={"type": "response"} -->
```http
HTTP/1.01 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.0"
off=8 version complete
off=8 error code=9 reason="Expected space after version"
```
-->

### Tab after HTTP version

<!-- meta={"type": "response"} -->
```http
HTTP/1.1\t200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=8 error code=9 reason="Expected space after version"
```

### CR before response and tab after HTTP version

<!-- meta={"type": "response"} -->
```http
\rHTTP/1.1\t200 OK


```

```log
off=1 message begin
off=1 len=4 span[protocol]="HTTP"
off=5 protocol complete
off=6 len=3 span[version]="1.1"
off=9 version complete
off=9 error code=9 reason="Expected space after version"
```

### Headers separated by CR

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Foo: 1\rBar: 2


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=3 span[header_field]="Foo"
off=21 header_field complete
off=22 len=1 span[header_value]="1"
off=24 error code=3 reason="Missing expected LF after header value"
```

### Invalid HTTP version

<!-- meta={"type": "response"} -->
```http
HTTP/5.6 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="5.6"
off=8 error code=9 reason="Invalid HTTP version"
```

## Invalid space after start line

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
 Host: foo
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=18 error code=30 reason="Unexpected space after start line"
```

### Extra space between HTTP version and status code

<!-- meta={"type": "response"} -->
```http
HTTP/1.1  200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=9 error code=13 reason="Invalid status code"
```

### Extra space between status code and reason

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200  OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=3 span[status]=" OK"
off=18 status complete
off=20 headers complete status=200 v=1/1 flags=0 content_length=0
```

### One-digit status code

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 2 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=10 error code=13 reason="Invalid status code"
```

### Only LFs present and no body

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK\nContent-Length: 0\n\n
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=16 error code=25 reason="Missing expected CR after response line"
```

### Only LFs present and no body (lenient)

<!-- meta={"type": "response-lenient-all"} -->
```http
HTTP/1.1 200 OK\nContent-Length: 0\n\n
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=16 status complete
off=16 len=14 span[header_field]="Content-Length"
off=31 header_field complete
off=32 len=1 span[header_value]="0"
off=34 header_value complete
off=35 headers complete status=200 v=1/1 flags=20 content_length=0
off=35 message complete
```

### Only LFs present

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK\n\
Foo: abc\n\
Bar: def\n\
\n\
BODY\n\
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=16 error code=25 reason="Missing expected CR after response line"
```

### Only LFs present (lenient)

<!-- meta={"type": "response-lenient-all"} -->
```http
HTTP/1.1 200 OK\n\
Foo: abc\n\
Bar: def\n\
\n\
BODY\n\
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=16 status complete
off=16 len=3 span[header_field]="Foo"
off=20 header_field complete
off=21 len=3 span[header_value]="abc"
off=25 header_value complete
off=25 len=3 span[header_field]="Bar"
off=29 header_field complete
off=30 len=3 span[header_value]="def"
off=34 header_value complete
off=35 headers complete status=200 v=1/1 flags=0 content_length=0
off=35 len=4 span[body]="BODY"
off=39 len=1 span[body]=lf
off=40 len=1 span[body]="\"
```

Finish
======

Those tests check the return codes and the behavior of `llhttp_finish()` C API.

## It should be safe to finish with cb after empty response

<!-- meta={"type": "response-finish"} -->
```http
HTTP/1.1 200 OK


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=19 headers complete status=200 v=1/1 flags=0 content_length=0
off=NULL finish=1
```

Content-Length header
=====================

## Response without `Content-Length`, but with body

The client should wait for the server's EOF. That is, when
`Content-Length` is not specified, and `Connection: close`, the end of body is
specified by the EOF.

_(Compare with APACHEBENCH_GET)_

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Date: Tue, 04 Aug 2009 07:59:32 GMT
Server: Apache
X-Powered-By: Servlet/2.5 JSP/2.1
Content-Type: text/xml; charset=utf-8
Connection: close

<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\">\n\
  <SOAP-ENV:Body>\n\
    <SOAP-ENV:Fault>\n\
       <faultcode>SOAP-ENV:Client</faultcode>\n\
       <faultstring>Client Error</faultstring>\n\
    </SOAP-ENV:Fault>\n\
  </SOAP-ENV:Body>\n\
</SOAP-ENV:Envelope>
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=4 span[header_field]="Date"
off=22 header_field complete
off=23 len=29 span[header_value]="Tue, 04 Aug 2009 07:59:32 GMT"
off=54 header_value complete
off=54 len=6 span[header_field]="Server"
off=61 header_field complete
off=62 len=6 span[header_value]="Apache"
off=70 header_value complete
off=70 len=12 span[header_field]="X-Powered-By"
off=83 header_field complete
off=84 len=19 span[header_value]="Servlet/2.5 JSP/2.1"
off=105 header_value complete
off=105 len=12 span[header_field]="Content-Type"
off=118 header_field complete
off=119 len=23 span[header_value]="text/xml; charset=utf-8"
off=144 header_value complete
off=144 len=10 span[header_field]="Connection"
off=155 header_field complete
off=156 len=5 span[header_value]="close"
off=163 header_value complete
off=165 headers complete status=200 v=1/1 flags=2 content_length=0
off=165 len=42 span[body]="<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
off=207 len=1 span[body]=lf
off=208 len=80 span[body]="<SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\">"
off=288 len=1 span[body]=lf
off=289 len=17 span[body]="  <SOAP-ENV:Body>"
off=306 len=1 span[body]=lf
off=307 len=20 span[body]="    <SOAP-ENV:Fault>"
off=327 len=1 span[body]=lf
off=328 len=45 span[body]="       <faultcode>SOAP-ENV:Client</faultcode>"
off=373 len=1 span[body]=lf
off=374 len=46 span[body]="       <faultstring>Client Error</faultstring>"
off=420 len=1 span[body]=lf
off=421 len=21 span[body]="    </SOAP-ENV:Fault>"
off=442 len=1 span[body]=lf
off=443 len=18 span[body]="  </SOAP-ENV:Body>"
off=461 len=1 span[body]=lf
off=462 len=20 span[body]="</SOAP-ENV:Envelope>"
```

## Content-Length-X

The header that starts with `Content-Length*` should not be treated as
`Content-Length`.

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Content-Length-X: 0
Transfer-Encoding: chunked

2
OK
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=16 span[header_field]="Content-Length-X"
off=34 header_field complete
off=35 len=1 span[header_value]="0"
off=38 header_value complete
off=38 len=17 span[header_field]="Transfer-Encoding"
off=56 header_field complete
off=57 len=7 span[header_value]="chunked"
off=66 header_value complete
off=68 headers complete status=200 v=1/1 flags=208 content_length=0
off=71 chunk header len=2
off=71 len=2 span[body]="OK"
off=75 chunk complete
off=78 chunk header len=0
off=80 chunk complete
off=80 message complete
```

## Content-Length reset when no body is received

<!-- meta={"type": "response", "skipBody": true} -->
```http
HTTP/1.1 200 OK
Content-Length: 123

HTTP/1.1 200 OK
Content-Length: 456


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=14 span[header_field]="Content-Length"
off=32 header_field complete
off=33 len=3 span[header_value]="123"
off=38 header_value complete
off=40 headers complete status=200 v=1/1 flags=20 content_length=123
off=40 skip body
off=40 message complete
off=40 reset
off=40 message begin
off=40 len=4 span[protocol]="HTTP"
off=44 protocol complete
off=45 len=3 span[version]="1.1"
off=48 version complete
off=53 len=2 span[status]="OK"
off=57 status complete
off=57 len=14 span[header_field]="Content-Length"
off=72 header_field complete
off=73 len=3 span[header_value]="456"
off=78 header_value complete
off=80 headers complete status=200 v=1/1 flags=20 content_length=456
off=80 skip body
off=80 message complete
```

Connection header
=================

## Proxy-Connection

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Content-Type: text/html; charset=UTF-8
Content-Length: 11
Proxy-Connection: close
Date: Thu, 31 Dec 2009 20:55:48 +0000

hello world
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=12 span[header_field]="Content-Type"
off=30 header_field complete
off=31 len=24 span[header_value]="text/html; charset=UTF-8"
off=57 header_value complete
off=57 len=14 span[header_field]="Content-Length"
off=72 header_field complete
off=73 len=2 span[header_value]="11"
off=77 header_value complete
off=77 len=16 span[header_field]="Proxy-Connection"
off=94 header_field complete
off=95 len=5 span[header_value]="close"
off=102 header_value complete
off=102 len=4 span[header_field]="Date"
off=107 header_field complete
off=108 len=31 span[header_value]="Thu, 31 Dec 2009 20:55:48 +0000"
off=141 header_value complete
off=143 headers complete status=200 v=1/1 flags=22 content_length=11
off=143 len=11 span[body]="hello world"
off=154 message complete
```

## HTTP/1.0 with keep-alive and EOF-terminated 200 status

There is no `Content-Length` in this response, so even though the
`keep-alive` is on - it should read until EOF.

<!-- meta={"type": "response"} -->
```http
HTTP/1.0 200 OK
Connection: keep-alive

HTTP/1.0 200 OK
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.0"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=10 span[header_field]="Connection"
off=28 header_field complete
off=29 len=10 span[header_value]="keep-alive"
off=41 header_value complete
off=43 headers complete status=200 v=1/0 flags=1 content_length=0
off=43 len=15 span[body]="HTTP/1.0 200 OK"
```

## HTTP/1.0 with keep-alive and 204 status

Responses with `204` status cannot have a body.

<!-- meta={"type": "response"} -->
```http
HTTP/1.0 204 No content
Connection: keep-alive

HTTP/1.0 200 OK
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.0"
off=8 version complete
off=13 len=10 span[status]="No content"
off=25 status complete
off=25 len=10 span[header_field]="Connection"
off=36 header_field complete
off=37 len=10 span[header_value]="keep-alive"
off=49 header_value complete
off=51 headers complete status=204 v=1/0 flags=1 content_length=0
off=51 message complete
off=51 reset
off=51 message begin
off=51 len=4 span[protocol]="HTTP"
off=55 protocol complete
off=56 len=3 span[version]="1.0"
off=59 version complete
off=64 len=2 span[status]="OK"
```

## HTTP/1.1 with EOF-terminated 200 status

There is no `Content-Length` in this response, so even though the
`keep-alive` is on (implicitly in HTTP 1.1) - it should read until EOF.

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK

HTTP/1.1 200 OK
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=19 headers complete status=200 v=1/1 flags=0 content_length=0
off=19 len=15 span[body]="HTTP/1.1 200 OK"
```

## HTTP/1.1 with 204 status

Responses with `204` status cannot have a body.

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 204 No content

HTTP/1.1 200 OK
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=10 span[status]="No content"
off=25 status complete
off=27 headers complete status=204 v=1/1 flags=0 content_length=0
off=27 message complete
off=27 reset
off=27 message begin
off=27 len=4 span[protocol]="HTTP"
off=31 protocol complete
off=32 len=3 span[version]="1.1"
off=35 version complete
off=40 len=2 span[status]="OK"
```

## HTTP/1.1 with keep-alive disabled and 204 status

<!-- meta={"type": "response" } -->
```http
HTTP/1.1 204 No content
Connection: close

HTTP/1.1 200 OK
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=10 span[status]="No content"
off=25 status complete
off=25 len=10 span[header_field]="Connection"
off=36 header_field complete
off=37 len=5 span[header_value]="close"
off=44 header_value complete
off=46 headers complete status=204 v=1/1 flags=2 content_length=0
off=46 message complete
off=47 error code=5 reason="Data after `Connection: close`"
```

## HTTP/1.1 with keep-alive disabled, content-length (lenient)

Parser should discard extra request in lenient mode.

<!-- meta={"type": "response-lenient-data-after-close" } -->
```http
HTTP/1.1 200 No content
Content-Length: 5
Connection: close

2ad731e3-4dcd-4f70-b871-0ad284b29ffc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=10 span[status]="No content"
off=25 status complete
off=25 len=14 span[header_field]="Content-Length"
off=40 header_field complete
off=41 len=1 span[header_value]="5"
off=44 header_value complete
off=44 len=10 span[header_field]="Connection"
off=55 header_field complete
off=56 len=5 span[header_value]="close"
off=63 header_value complete
off=65 headers complete status=200 v=1/1 flags=22 content_length=5
off=65 len=5 span[body]="2ad73"
off=70 message complete
```

## HTTP/1.1 with keep-alive disabled, content-length

Parser should discard extra request in strict mode.

<!-- meta={"type": "response" } -->
```http
HTTP/1.1 200 No content
Content-Length: 5
Connection: close

2ad731e3-4dcd-4f70-b871-0ad284b29ffc
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=10 span[status]="No content"
off=25 status complete
off=25 len=14 span[header_field]="Content-Length"
off=40 header_field complete
off=41 len=1 span[header_value]="5"
off=44 header_value complete
off=44 len=10 span[header_field]="Connection"
off=55 header_field complete
off=56 len=5 span[header_value]="close"
off=63 header_value complete
off=65 headers complete status=200 v=1/1 flags=22 content_length=5
off=65 len=5 span[body]="2ad73"
off=70 message complete
off=71 error code=5 reason="Data after `Connection: close`"
```

## HTTP/1.1 with keep-alive disabled and 204 status (lenient)

<!-- meta={"type": "response-lenient-keep-alive"} -->
```http
HTTP/1.1 204 No content
Connection: close

HTTP/1.1 200 OK
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=10 span[status]="No content"
off=25 status complete
off=25 len=10 span[header_field]="Connection"
off=36 header_field complete
off=37 len=5 span[header_value]="close"
off=44 header_value complete
off=46 headers complete status=204 v=1/1 flags=2 content_length=0
off=46 message complete
off=46 reset
off=46 message begin
off=46 len=4 span[protocol]="HTTP"
off=50 protocol complete
off=51 len=3 span[version]="1.1"
off=54 version complete
off=59 len=2 span[status]="OK"
```

## HTTP 101 response with Upgrade and Content-Length header

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 101 Switching Protocols
Connection: upgrade
Upgrade: h2c
Content-Length: 4

body\
proto
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=19 span[status]="Switching Protocols"
off=34 status complete
off=34 len=10 span[header_field]="Connection"
off=45 header_field complete
off=46 len=7 span[header_value]="upgrade"
off=55 header_value complete
off=55 len=7 span[header_field]="Upgrade"
off=63 header_field complete
off=64 len=3 span[header_value]="h2c"
off=69 header_value complete
off=69 len=14 span[header_field]="Content-Length"
off=84 header_field complete
off=85 len=1 span[header_value]="4"
off=88 header_value complete
off=90 headers complete status=101 v=1/1 flags=34 content_length=4
off=90 message complete
off=90 error code=22 reason="Pause on CONNECT/Upgrade"
```

## HTTP 101 response with Upgrade and Transfer-Encoding header

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 101 Switching Protocols
Connection: upgrade
Upgrade: h2c
Transfer-Encoding: chunked

2
bo
2
dy
0

proto
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=19 span[status]="Switching Protocols"
off=34 status complete
off=34 len=10 span[header_field]="Connection"
off=45 header_field complete
off=46 len=7 span[header_value]="upgrade"
off=55 header_value complete
off=55 len=7 span[header_field]="Upgrade"
off=63 header_field complete
off=64 len=3 span[header_value]="h2c"
off=69 header_value complete
off=69 len=17 span[header_field]="Transfer-Encoding"
off=87 header_field complete
off=88 len=7 span[header_value]="chunked"
off=97 header_value complete
off=99 headers complete status=101 v=1/1 flags=21c content_length=0
off=99 message complete
off=99 error code=22 reason="Pause on CONNECT/Upgrade"
```

## HTTP 200 response with Upgrade header

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Connection: upgrade
Upgrade: h2c

body
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=10 span[header_field]="Connection"
off=28 header_field complete
off=29 len=7 span[header_value]="upgrade"
off=38 header_value complete
off=38 len=7 span[header_field]="Upgrade"
off=46 header_field complete
off=47 len=3 span[header_value]="h2c"
off=52 header_value complete
off=54 headers complete status=200 v=1/1 flags=14 content_length=0
off=54 len=4 span[body]="body"
```

## HTTP 200 response with Upgrade header and Content-Length

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Connection: upgrade
Upgrade: h2c
Content-Length: 4

body
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=10 span[header_field]="Connection"
off=28 header_field complete
off=29 len=7 span[header_value]="upgrade"
off=38 header_value complete
off=38 len=7 span[header_field]="Upgrade"
off=46 header_field complete
off=47 len=3 span[header_value]="h2c"
off=52 header_value complete
off=52 len=14 span[header_field]="Content-Length"
off=67 header_field complete
off=68 len=1 span[header_value]="4"
off=71 header_value complete
off=73 headers complete status=200 v=1/1 flags=34 content_length=4
off=73 len=4 span[body]="body"
off=77 message complete
```

## HTTP 200 response with Upgrade header and Transfer-Encoding

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 200 OK
Connection: upgrade
Upgrade: h2c
Transfer-Encoding: chunked

2
bo
2
dy
0


```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=2 span[status]="OK"
off=17 status complete
off=17 len=10 span[header_field]="Connection"
off=28 header_field complete
off=29 len=7 span[header_value]="upgrade"
off=38 header_value complete
off=38 len=7 span[header_field]="Upgrade"
off=46 header_field complete
off=47 len=3 span[header_value]="h2c"
off=52 header_value complete
off=52 len=17 span[header_field]="Transfer-Encoding"
off=70 header_field complete
off=71 len=7 span[header_value]="chunked"
off=80 header_value complete
off=82 headers complete status=200 v=1/1 flags=21c content_length=0
off=85 chunk header len=2
off=85 len=2 span[body]="bo"
off=89 chunk complete
off=92 chunk header len=2
off=92 len=2 span[body]="dy"
off=96 chunk complete
off=99 chunk header len=0
off=101 chunk complete
off=101 message complete
```

## HTTP 304 with Content-Length

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 304 Not Modified
Content-Length: 10


HTTP/1.1 200 OK
Content-Length: 5

hello
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=12 span[status]="Not Modified"
off=27 status complete
off=27 len=14 span[header_field]="Content-Length"
off=42 header_field complete
off=43 len=2 span[header_value]="10"
off=47 header_value complete
off=49 headers complete status=304 v=1/1 flags=20 content_length=10
off=49 message complete
off=51 reset
off=51 message begin
off=51 len=4 span[protocol]="HTTP"
off=55 protocol complete
off=56 len=3 span[version]="1.1"
off=59 version complete
off=64 len=2 span[status]="OK"
off=68 status complete
off=68 len=14 span[header_field]="Content-Length"
off=83 header_field complete
off=84 len=1 span[header_value]="5"
off=87 header_value complete
off=89 headers complete status=200 v=1/1 flags=20 content_length=5
off=89 len=5 span[body]="hello"
off=94 message complete
```

## HTTP 304 with Transfer-Encoding

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 304 Not Modified
Transfer-Encoding: chunked

HTTP/1.1 200 OK
Transfer-Encoding: chunked

5
hello
0

```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=12 span[status]="Not Modified"
off=27 status complete
off=27 len=17 span[header_field]="Transfer-Encoding"
off=45 header_field complete
off=46 len=7 span[header_value]="chunked"
off=55 header_value complete
off=57 headers complete status=304 v=1/1 flags=208 content_length=0
off=57 message complete
off=57 reset
off=57 message begin
off=57 len=4 span[protocol]="HTTP"
off=61 protocol complete
off=62 len=3 span[version]="1.1"
off=65 version complete
off=70 len=2 span[status]="OK"
off=74 status complete
off=74 len=17 span[header_field]="Transfer-Encoding"
off=92 header_field complete
off=93 len=7 span[header_value]="chunked"
off=102 header_value complete
off=104 headers complete status=200 v=1/1 flags=208 content_length=0
off=107 chunk header len=5
off=107 len=5 span[body]="hello"
off=114 chunk complete
off=117 chunk header len=0
```

## HTTP 100 first, then 400

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 100 Continue


HTTP/1.1 404 Not Found
Content-Type: text/plain; charset=utf-8
Content-Length: 14
Date: Fri, 15 Sep 2023 19:47:23 GMT
Server: Python/3.10 aiohttp/4.0.0a2.dev0

404: Not Found
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=8 span[status]="Continue"
off=23 status complete
off=25 headers complete status=100 v=1/1 flags=0 content_length=0
off=25 message complete
off=27 reset
off=27 message begin
off=27 len=4 span[protocol]="HTTP"
off=31 protocol complete
off=32 len=3 span[version]="1.1"
off=35 version complete
off=40 len=9 span[status]="Not Found"
off=51 status complete
off=51 len=12 span[header_field]="Content-Type"
off=64 header_field complete
off=65 len=25 span[header_value]="text/plain; charset=utf-8"
off=92 header_value complete
off=92 len=14 span[header_field]="Content-Length"
off=107 header_field complete
off=108 len=2 span[header_value]="14"
off=112 header_value complete
off=112 len=4 span[header_field]="Date"
off=117 header_field complete
off=118 len=29 span[header_value]="Fri, 15 Sep 2023 19:47:23 GMT"
off=149 header_value complete
off=149 len=6 span[header_field]="Server"
off=156 header_field complete
off=157 len=32 span[header_value]="Python/3.10 aiohttp/4.0.0a2.dev0"
off=191 header_value complete
off=193 headers complete status=404 v=1/1 flags=20 content_length=14
off=193 len=14 span[body]="404: Not Found"
off=207 message complete
```

## HTTP 103 first, then 200

<!-- meta={"type": "response"} -->
```http
HTTP/1.1 103 Early Hints
Link: </styles.css>; rel=preload; as=style

HTTP/1.1 200 OK
Date: Wed, 13 Sep 2023 11:09:41 GMT
Connection: keep-alive
Keep-Alive: timeout=5
Content-Length: 17

response content
```

```log
off=0 message begin
off=0 len=4 span[protocol]="HTTP"
off=4 protocol complete
off=5 len=3 span[version]="1.1"
off=8 version complete
off=13 len=11 span[status]="Early Hints" 
off=26 status complete
off=26 len=4 span[header_field]="Link"
off=31 header_field complete
off=32 len=36 span[header_value]="</styles.css>; rel=preload; as=style"
off=70 header_value complete
off=72 headers complete status=103 v=1/1 flags=0 content_length=0
off=72 message complete
off=72 reset
off=72 message begin
off=72 len=4 span[protocol]="HTTP"
off=76 protocol complete
off=77 len=3 span[version]="1.1"
off=80 version complete
off=85 len=2 span[status]="OK"
off=89 status complete
off=89 len=4 span[header_field]="Date"
off=94 header_field complete
off=95 len=29 span[header_value]="Wed, 13 Sep 2023 11:09:41 GMT"
off=126 header_value complete
off=126 len=10 span[header_field]="Connection"
off=137 header_field complete
off=138 len=10 span[header_value]="keep-alive"
off=150 header_value complete
off=150 len=10 span[header_field]="Keep-Alive"
off=161 header_field complete
off=162 len=9 span[header_value]="timeout=5"
off=173 header_value complete
off=173 len=14 span[header_field]="Content-Length"
off=188 header_field complete
off=189 len=2 span[header_value]="17"
off=193 header_value complete
off=195 headers complete status=200 v=1/1 flags=21 content_length=17
off=195 len=16 span[body]="response content"
```



