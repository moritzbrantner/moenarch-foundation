# math-sparse-data-server

Thin HTTP API adapter for `moritzbrantner-math-sparse-data`.

Run:

```bash
cargo run -p math-sparse-data-server -- --addr 127.0.0.1:3000
```

Endpoints:

- `GET /health`
- `GET /api/package`
- `GET /api/schema`
- `GET /api/operations`
- `POST /api/run`
- `POST /api/<operation-id>`
