CREATE TABLE IF NOT EXISTS vectors (
    id        TEXT    NOT NULL,
    namespace TEXT    NOT NULL,
    dimension INTEGER NOT NULL,
    vector    BLOB    NOT NULL,
    metadata  TEXT,
    PRIMARY KEY (namespace, id)
);

CREATE INDEX IF NOT EXISTS idx_vectors_namespace ON vectors (namespace);
