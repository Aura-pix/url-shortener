# Rust URL Shortener

A small URL shortener built with Axum. Links are stored in memory, so they are
cleared whenever the process restarts.

## Run

Install Rust and Cargo, then start the server:

```sh
cargo run
```

The server listens on `http://localhost:3000`. Set `BASE_URL` if the short URLs
should use a different externally reachable origin.

## API

### `POST /api/shorten`

Accepts JSON matching the `CreateLinkRequest` serde struct:

```rust
pub struct CreateLinkRequest {
    pub long_url: String,
}
```

The URL must be a valid HTTP or HTTPS URL. On success, the handler returns
`201 Created` with JSON matching `LinkResponse`:

```rust
pub struct LinkResponse {
    pub short_url: String,
    pub created_at: DateTime<Utc>,
    pub hits: u64,
}
```

Example request:

```json
{
  "long_url": "https://example.com/articles/rust"
}
```

Example response:

```json
{
  "short_url": "http://localhost:3000/api/links/1",
  "created_at": "2026-11-25T00:00:00Z",
  "hits": 0
}
```

`created_at` is serialized as an RFC 3339 UTC timestamp. `hits` starts at zero
and increases for each redirect.

### `GET /api/links/{id}`

Redirects to the saved long URL with `302 Found` and increments its hit count.
Returns `404 Not Found` when the ID does not exist.

Invalid JSON or a URL that is not HTTP(S) returns `400 Bad Request`. Unexpected
server errors return `500 Internal Server Error`.
