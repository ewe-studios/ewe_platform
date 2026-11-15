# MIME data

`mime.json` maps a file extension to a media type. `foundation_netio::shared::mime`
loads it at compile time with `foundation_macros::include_json!`.

## Source

| Item | Value |
|---|---|
| Source | `https://raw.githubusercontent.com/jshttp/mime-db/v1.54.0/db.json` |
| Project | `jshttp/mime-db` |
| Version | 1.54.0 |
| Download date | 2026-11-16 |
| Media types in the source | 2,522 |
| Extensions in the result | 1,239 |
| Ambiguous extensions in the source | 48 |

`mime.json` is a vendored copy. Continuous integration (CI) never downloads it.
The file is committed, so a build is reproducible without network access.

## The selection rule

One extension can come from more than one media type. The generator resolves the
ambiguity with a written rule, never with the file order:

1. Prefer the candidate whose `source` is `"iana"`.
2. Then prefer a candidate with no `source` key.
3. Then prefer a candidate whose `source` is `"apache"`.
4. Then prefer any other candidate.
5. Inside one rank, prefer the alphabetically first media type.

## Regenerate the file

Download the source into one empty directory. Then run the generator. The
generator needs Python 3 and no other package.

```bash
curl -sSLO https://raw.githubusercontent.com/jshttp/mime-db/v1.54.0/db.json
python3 -I generate_mime.py db.json mime.json
```

Update the version and the download date in this file when the source version
changes. A change to the source gives a diff of data rows only.

## Change the data by hand

Do not edit `mime.json` by hand. Change `generate_mime.py`, regenerate the file,
and update this file. The test in `tests/mime_data_tests.rs` checks the shape
and the known rows.
