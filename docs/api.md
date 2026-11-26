# URL Shortener API

The API listens on port `3000`. Short links use the base URL configured by the
`BASE_URL` environment variable; when it is unset, the base URL is
`http://localhost:3000`.

## `POST /api/shorten`

Create a short link by sending a JSON request:

```json
{
  "long_url": "https://example.com/articles/rust"
}
```

The request is deserialized into `CreateLinkRequest`:

| Field | Type | Description |
| --- | --- | --- |
| `long_url` | `string` | The destination URL. Must be a valid URL with the `http` or `https` scheme and a host. |

On success, the endpoint responds with `201 Created` and a JSON `LinkResponse`:

```json
{
  "short_url": "http://localhost:3000/api/links/1",
  "created_at": "2026-11-25T00:00:00Z",
  "hits": 0
}
```

| Field | Type | Description |
| --- | --- | --- |
| `short_url` | `string` | The generated redirect URL. |
| `created_at` | `DateTime<Utc>` (RFC 3339 JSON string) | The time the link was created, in UTC. |
| `hits` | `u64` | The redirect count, initially `0`. |

Malformed JSON, an invalid URL, or a URL that is not HTTP(S) returns
`400 Bad Request` with a JSON error object:

```json
{
  "error": "long_url must be a valid HTTP or HTTPS URL"
}
```

The error message may describe the JSON parsing failure when the request body
cannot be deserialized.

## `GET /api/links/:id`

Redirect to the destination for the specified short-link ID. Each successful
lookup increments that link's hit count and returns `302 Found` with a `Location`
header containing the destination URL. There is no JSON response body.

An unknown ID returns `404 Not Found` with:

```json
{
  "error": "link not found"
}
```

Unexpected internal failures return `500 Internal Server Error` with:

```json
{
  "error": "internal server error"
}
```

## Storage

Links and hit counts are held in memory only. They are lost when the server
restarts, and links are not shared across separate server processes.
