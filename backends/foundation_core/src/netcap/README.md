# HTTP Specifications
Built from the [HTTP](https://datatracker.ietf.org/doc/html/rfc7230) specification but its evolving and has not yet covered the whole specification.

## HTTP Specifications (Area of Concerns)

[3.2.4](https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.4).  Field Parsing

   Messages are parsed using a generic algorithm, independent of the
   individual header field names.  The contents within a given field
   value are not parsed until a later stage of message interpretation
   (usually after the message's entire header section has been
   processed).  Consequently, this specification does not use ABNF rules
   to define each "Field-Name: Field Value" pair, as was done in
   previous editions.  Instead, this specification uses ABNF rules that
   are named according to each registered field name, wherein the rule
   defines the valid grammar for that field's corresponding field values
   (i.e., after the field-value has been extracted from the header
   section by a generic field parser).

   No whitespace is allowed between the header field-name and colon.  In
   the past, differences in the handling of such whitespace have led to
   security vulnerabilities in request routing and response handling.  A
   server MUST reject any received request message that contains
   whitespace between a header field-name and colon with a response code
   of 400 (Bad Request).  A proxy MUST remove any such whitespace from a
   response message before forwarding the message downstream.

   A field value might be preceded and/or followed by optional
   whitespace (OWS); a single SP preceding the field-value is preferred
   for consistent readability by humans.  The field value does not
   include any leading or trailing whitespace: OWS occurring before the
   first non-whitespace octet of the field value or after the last
   non-whitespace octet of the field value ought to be excluded by
   parsers when extracting the field value from a header field.

   Historically, HTTP header field values could be extended over
   multiple lines by preceding each extra line with at least one space
   or horizontal tab (obs-fold).  This specification deprecates such
   line folding except within the message/http media type
   (Section 8.3.1).  A sender MUST NOT generate a message that includes
   line folding (i.e., that has any field-value that contains a match to
   the obs-fold rule) unless the message is intended for packaging
   within the message/http media type.

[3.1.1](https://datatracker.ietf.org/doc/html/rfc7230#section-3.1.1).  Request Line

   A request-line begins with a method token, followed by a single space
   (SP), the request-target, another single space (SP), the protocol
   version, and ends with CRLF.

     request-line   = method SP request-target SP HTTP-version CRLF

   The method token indicates the request method to be performed on the
   target resource.  The request method is case-sensitive.

     method         = token

   The request methods defined by this specification can be found in
   Section 4 of [RFC7231], along with information regarding the HTTP
   method registry and considerations for defining new methods.

   The request-target identifies the target resource upon which to apply
   the request, as defined in Section 5.3.

   Recipients typically parse the request-line into its component parts
   by splitting on whitespace (see Section 3.5), since no whitespace is
   allowed in the three components.  Unfortunately, some user agents
   fail to properly encode or exclude whitespace found in hypertext
   references, resulting in those disallowed characters being sent in a
   request-target.

   Recipients of an invalid request-line SHOULD respond with either a
   400 (Bad Request) error or a 301 (Moved Permanently) redirect with
   the request-target properly encoded.  A recipient SHOULD NOT attempt
   to autocorrect and then process the request without a redirect, since
   the invalid request-line might be deliberately crafted to bypass
   security filters along the request chain.

   HTTP does not place a predefined limit on the length of a
   request-line, as described in Section 2.5.  A server that receives a
   method longer than any that it implements SHOULD respond with a 501
   (Not Implemented) status code.  A server that receives a
   request-target longer than any URI it wishes to parse MUST respond
   with a 414 (URI Too Long) status code (see Section 6.5.12 of
   [RFC7231]).

   Various ad hoc limitations on request-line length are found in
   practice.  It is RECOMMENDED that all HTTP senders and recipients
   support, at a minimum, request-line lengths of 8000 octets.

See [HTTP Security Considerations](https://datatracker.ietf.org/doc/html/rfc7230#section-9)

9.0 Security Considerations

   This section is meant to inform developers, information providers,
   and users of known security considerations relevant to HTTP message
   syntax, parsing, and routing.  Security considerations about HTTP
   semantics and payloads are addressed in [RFC7231].

9.1.  Establishing Authority

   HTTP relies on the notion of an authoritative response: a response
   that has been determined by (or at the direction of) the authority
   identified within the target URI to be the most appropriate response
   for that request given the state of the target resource at the time
   of response message origination.  Providing a response from a
   non-authoritative source, such as a shared cache, is often useful to
   improve performance and availability, but only to the extent that the
   source can be trusted or the distrusted response can be safely used.

   Unfortunately, establishing authority can be difficult.  For example,
   phishing is an attack on the user's perception of authority, where
   that perception can be misled by presenting similar branding in



Fielding & Reschke           Standards Track                   [Page 67]

RFC 7230           HTTP/1.1 Message Syntax and Routing         June 2014


   hypertext, possibly aided by userinfo obfuscating the authority
   component (see Section 2.7.1).  User agents can reduce the impact of
   phishing attacks by enabling users to easily inspect a target URI
   prior to making an action, by prominently distinguishing (or
   rejecting) userinfo when present, and by not sending stored
   credentials and cookies when the referring document is from an
   unknown or untrusted source.

   When a registered name is used in the authority component, the "http"
   URI scheme (Section 2.7.1) relies on the user's local name resolution
   service to determine where it can find authoritative responses.  This
   means that any attack on a user's network host table, cached names,
   or name resolution libraries becomes an avenue for attack on
   establishing authority.  Likewise, the user's choice of server for
   Domain Name Service (DNS), and the hierarchy of servers from which it
   obtains resolution results, could impact the authenticity of address
   mappings; DNS Security Extensions (DNSSEC, [RFC4033]) are one way to
   improve authenticity.

   Furthermore, after an IP address is obtained, establishing authority
   for an "http" URI is vulnerable to attacks on Internet Protocol
   routing.

   The "https" scheme (Section 2.7.2) is intended to prevent (or at
   least reveal) many of these potential attacks on establishing
   authority, provided that the negotiated TLS connection is secured and
   the client properly verifies that the communicating server's identity
   matches the target URI's authority component (see [RFC2818]).
   Correctly implementing such verification can be difficult (see
   [Georgiev]).

9.2.  Risks of Intermediaries

   By their very nature, HTTP intermediaries are men-in-the-middle and,
   thus, represent an opportunity for man-in-the-middle attacks.
   Compromise of the systems on which the intermediaries run can result
   in serious security and privacy problems.  Intermediaries might have
   access to security-related information, personal information about
   individual users and organizations, and proprietary information
   belonging to users and content providers.  A compromised
   intermediary, or an intermediary implemented or configured without
   regard to security and privacy considerations, might be used in the
   commission of a wide range of potential attacks.

   Intermediaries that contain a shared cache are especially vulnerable
   to cache poisoning attacks, as described in Section 8 of [RFC7234].





Fielding & Reschke           Standards Track                   [Page 68]

RFC 7230           HTTP/1.1 Message Syntax and Routing         June 2014


   Implementers need to consider the privacy and security implications
   of their design and coding decisions, and of the configuration
   options they provide to operators (especially the default
   configuration).

   Users need to be aware that intermediaries are no more trustworthy
   than the people who run them; HTTP itself cannot solve this problem.

9.3.  Attacks via Protocol Element Length

   Because HTTP uses mostly textual, character-delimited fields, parsers
   are often vulnerable to attacks based on sending very long (or very
   slow) streams of data, particularly where an implementation is
   expecting a protocol element with no predefined length.

   To promote interoperability, specific recommendations are made for
   minimum size limits on request-line (Section 3.1.1) and header fields
   (Section 3.2).  These are minimum recommendations, chosen to be
   supportable even by implementations with limited resources; it is
   expected that most implementations will choose substantially higher
   limits.

   A server can reject a message that has a request-target that is too
   long (Section 6.5.12 of [RFC7231]) or a request payload that is too
   large (Section 6.5.11 of [RFC7231]).  Additional status codes related
   to capacity limits have been defined by extensions to HTTP [RFC6585].

   Recipients ought to carefully limit the extent to which they process
   other protocol elements, including (but not limited to) request
   methods, response status phrases, header field-names, numeric values,
   and body chunks.  Failure to limit such processing can result in
   buffer overflows, arithmetic overflows, or increased vulnerability to
   denial-of-service attacks.

9.4.  Response Splitting

   Response splitting (a.k.a, CRLF injection) is a common technique,
   used in various attacks on Web usage, that exploits the line-based
   nature of HTTP message framing and the ordered association of
   requests to responses on persistent connections [Klein].  This
   technique can be particularly damaging when the requests pass through
   a shared cache.

   Response splitting exploits a vulnerability in servers (usually
   within an application server) where an attacker can send encoded data
   within some parameter of the request that is later decoded and echoed
   within any of the response header fields of the response.  If the
   decoded data is crafted to look like the response has ended and a



Fielding & Reschke           Standards Track                   [Page 69]

RFC 7230           HTTP/1.1 Message Syntax and Routing         June 2014


   subsequent response has begun, the response has been split and the
   content within the apparent second response is controlled by the
   attacker.  The attacker can then make any other request on the same
   persistent connection and trick the recipients (including
   intermediaries) into believing that the second half of the split is
   an authoritative answer to the second request.

   For example, a parameter within the request-target might be read by
   an application server and reused within a redirect, resulting in the
   same parameter being echoed in the Location header field of the
   response.  If the parameter is decoded by the application and not
   properly encoded when placed in the response field, the attacker can
   send encoded CRLF octets and other content that will make the
   application's single response look like two or more responses.

   A common defense against response splitting is to filter requests for
   data that looks like encoded CR and LF (e.g., "%0D" and "%0A").
   However, that assumes the application server is only performing URI
   decoding, rather than more obscure data transformations like charset
   transcoding, XML entity translation, base64 decoding, sprintf
   reformatting, etc.  A more effective mitigation is to prevent
   anything other than the server's core protocol libraries from sending
   a CR or LF within the header section, which means restricting the
   output of header fields to APIs that filter for bad octets and not
   allowing application servers to write directly to the protocol
   stream.

9.5.  Request Smuggling

   Request smuggling ([Linhart]) is a technique that exploits
   differences in protocol parsing among various recipients to hide
   additional requests (which might otherwise be blocked or disabled by
   policy) within an apparently harmless request.  Like response
   splitting, request smuggling can lead to a variety of attacks on HTTP
   usage.

   This specification has introduced new requirements on request
   parsing, particularly with regard to message framing in
   Section 3.3.3, to reduce the effectiveness of request smuggling.

9.6.  Message Integrity

   HTTP does not define a specific mechanism for ensuring message
   integrity, instead relying on the error-detection ability of
   underlying transport protocols and the use of length or
   chunk-delimited framing to detect completeness.  Additional integrity
   mechanisms, such as hash functions or digital signatures applied to
   the content, can be selectively added to messages via extensible



Fielding & Reschke           Standards Track                   [Page 70]

RFC 7230           HTTP/1.1 Message Syntax and Routing         June 2014


   metadata header fields.  Historically, the lack of a single integrity
   mechanism has been justified by the informal nature of most HTTP
   communication.  However, the prevalence of HTTP as an information
   access mechanism has resulted in its increasing use within
   environments where verification of message integrity is crucial.

   User agents are encouraged to implement configurable means for
   detecting and reporting failures of message integrity such that those
   means can be enabled within environments for which integrity is
   necessary.  For example, a browser being used to view medical history
   or drug interaction information needs to indicate to the user when
   such information is detected by the protocol to be incomplete,
   expired, or corrupted during transfer.  Such mechanisms might be
   selectively enabled via user agent extensions or the presence of
   message integrity metadata in a response.  At a minimum, user agents
   ought to provide some indication that allows a user to distinguish
   between a complete and incomplete response message (Section 3.4) when
   such verification is desired.

9.7.  Message Confidentiality

   HTTP relies on underlying transport protocols to provide message
   confidentiality when that is desired.  HTTP has been specifically
   designed to be independent of the transport protocol, such that it
   can be used over many different forms of encrypted connection, with
   the selection of such transports being identified by the choice of
   URI scheme or within user agent configuration.

   The "https" scheme can be used to identify resources that require a
   confidential connection, as described in Section 2.7.2.

9.8.  Privacy of Server Log Information

   A server is in the position to save personal data about a user's
   requests over time, which might identify their reading patterns or
   subjects of interest.  In particular, log information gathered at an
   intermediary often contains a history of user agent interaction,
   across a multitude of sites, that can be traced to individual users.

   HTTP log information is confidential in nature; its handling is often
   constrained by laws and regulations.  Log information needs to be
   securely stored and appropriate guidelines followed for its analysis.
   Anonymization of personal information within individual entries
   helps, but it is generally not sufficient to prevent real log traces
   from being re-identified based on correlation with other access
   characteristics.  As such, access traces that are keyed to a specific
   client are unsafe to publish even if the key is pseudonymous.




Fielding & Reschke           Standards Track                   [Page 71]

RFC 7230           HTTP/1.1 Message Syntax and Routing         June 2014


   To minimize the risk of theft or accidental publication, log
   information ought to be purged of personally identifiable
   information, including user identifiers, IP addresses, and
   user-provided query parameters, as soon as that information is no
   longer necessary to support operational needs for security, auditing,
   or fraud control.
