//! Tests extracted from shared/web_conn.rs
mod tests {
    use foundation_http::shared::web_conn::*;

    #[test]
    fn test_web_conn_default() {
        let conn = WebConn::default();
        assert_eq!(conn.status, 200);
        assert!(conn.headers.is_empty());
        assert!(conn.body.is_none());
    }

    #[test]
    fn test_web_conn_set_status() {
        let mut conn = WebConn::new();
        conn.set_status(302);
        assert_eq!(conn.status, 302);
    }

    #[test]
    fn test_web_conn_set_header() {
        let mut conn = WebConn::new();
        conn.set_header("Content-Type", "text/html");
        assert_eq!(conn.headers.len(), 1);
        assert_eq!(conn.headers[0], ("Content-Type".into(), "text/html".into()));
    }

    #[test]
    fn test_web_conn_set_header_replaces() {
        let mut conn = WebConn::new();
        conn.set_header("X-Request-Id", "old");
        conn.set_header("X-Request-Id", "new");
        assert_eq!(conn.headers.len(), 1);
        assert_eq!(conn.headers[0], ("X-Request-Id".into(), "new".into()));
    }

    #[test]
    fn test_web_conn_append_header() {
        let mut conn = WebConn::new();
        conn.append_header("Set-Cookie", "a=1");
        conn.append_header("Set-Cookie", "b=2");
        assert_eq!(conn.headers.len(), 2);
    }

    #[test]
    fn test_web_conn_set_body() {
        let mut conn = WebConn::new();
        conn.set_body(br#"{"ok":true}"#.to_vec());
        assert_eq!(conn.body, Some(br#"{"ok":true}"#.to_vec()));
    }

    #[test]
    fn test_web_conn_full_response() {
        let mut conn = WebConn::new();
        conn.set_status(201);
        conn.set_header("Location", "/items/42");
        conn.set_body(br#"{"id":42}"#.to_vec());
        assert_eq!(conn.status, 201);
        assert_eq!(conn.headers.len(), 1);
        assert_eq!(conn.body, Some(br#"{"id":42}"#.to_vec()));
    }

    #[test]
    fn test_web_conn_various_statuses() {
        let statuses = [200, 201, 204, 301, 302, 400, 401, 403, 404, 500, 502, 503];
        for s in statuses {
            let mut conn = WebConn::new();
            conn.set_status(s);
            assert_eq!(conn.status, s);
        }
    }
}
